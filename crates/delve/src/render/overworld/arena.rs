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

pub mod art_v2;
mod barony;
pub(super) mod college;
pub(super) mod ranch;
mod bones;
pub mod class_looks;
mod bounties;
mod cards;
mod countryside;
mod draw_list;
mod coastal;
mod woodland;
mod wetland;
mod aerie;
mod mummers;
mod hippodrome;
mod windcountry;
mod bowcountry;
mod town;
mod nooks;
mod props;
mod eastern;
mod feel;
mod northern;
mod critters;
mod forest;
mod fortune;
mod frogs;
mod gate;
mod hand;
mod hireling;
pub(super) mod holdings;
mod home;
mod joust;
mod kingdom;
pub mod knights;
mod lessons;
mod marks;
mod signatures;
mod merchant;
mod pastimes;
pub mod plates;
mod pitstop;
pub mod race3d;
mod votes;
mod rescues;
mod runes;
mod secrets;
mod side;
pub mod sprites;
mod stage;
mod stature;
mod strike;
mod tavern;
pub(super) mod three;
mod tide;
pub mod tray;
mod trophies;
mod ults;
mod void;
mod volleys;
mod wanted;
mod works;
mod world;
mod yard;

pub use cards::card_art;
#[cfg(any(test, feature = "testing"))]
pub use hippodrome::race_camera;

/// The Delve's folk, for the loop's crawl to meet in its halls.
pub fn crawl_figure(who: &str) -> Option<Img> {
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
pub fn crawl_enemy(kind: crate::rules::EnemyKind, tick: u32) -> Img {
    sprites::enemy(kind, tick)
}

/// A standing thing of the Delve's halls, for the crawl: a tomb, a rock, a
/// mushroom, an eye-pillar, a fire.
/// The frog sprites, for a sheet: frog, hopping, perched, a puff at three
/// stages, and the three figures.
#[cfg(any(test, feature = "testing"))]
pub fn frog_sprites(tick: u32) -> Vec<Img> {
    use crate::rules::frogs::Gift;
    let mut out = vec![
        frogs::frog(tick, false, false),
        frogs::frog(tick, true, false),
        frogs::perched(false),
    ];
    for age in [4, 10, 16] {
        out.push(frogs::transform(Gift::Princess, age, tick).1.unwrap());
    }
    for gift in [Gift::Princess, Gift::Prince, Gift::Clown] {
        out.push(frogs::transform(gift, 60, tick).0);
    }
    out
}

/// A critter at a stage of its move, for a sheet.
#[cfg(any(test, feature = "testing"))]
pub fn critter_figure(
    kind: crate::rules::EnemyKind,
    stage: u8,
    timer: u32,
    tick: u32,
) -> Img {
    critters::figure(kind, stage, timer, tick)
}

pub fn crawl_prop(what: &str, tick: u32) -> Img {
    match what {
        "tomb" => sprites::tomb(1),
        "rock" => sprites::rock(),
        "mushroom" => sprites::mushroom(tick / 40),
        "eye_pillar" => sprites::eye_pillar(1, tick / 3),
        _ => sprites::brazier(true, tick, 7),
    }
}
/// The crawl uses the same compact room signs as the playable arena.
pub fn crawl_sign(label: &str) -> Img {
    super::kit::district_sign(label)
}

pub use knights::of_kit as knight_of;

use draw_list::{DrawList, Layer};
use std::sync::OnceLock;

use super::ink::{BLACK, Img, PALETTE, Rgb, bayer, hash, is_signal, rgb, vnoise};
use super::kit::{RockKind, Tiles, chest, district_sign, torch};
use super::light::{DUSK, Light, STEPS, light_at, step_down, warm};
use crate::rules::{imps, looks};
use crate::rules::{
    BOMB_REARM, COLS, EnemyKind, HEIGHT, Pack, Phase, ROWS, Room, RoomKind, Run, TELEGRAPH,
    TILE_UNITS, Tile, Trap, VIGIL_REACH, WIDTH, ledge::FOOT,
};

/// Pixels per tile: the overworld's.
const TILE: i32 = 16;
/// Pixels per arena unit.
pub const UNIT: f32 = TILE as f32 / TILE_UNITS;
/// The native view: one screen of room. A great hall is drawn whole and the
/// camera shows this much of it.
pub const NATIVE_W: i32 = COLS as i32 * TILE;
pub const NATIVE_H: i32 = ROWS as i32 * TILE;

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
        self.blit_ranked(scenery, figures, out, None);
    }

    fn blit_ranked(
        &self,
        scenery: &Img,
        figures: &Img,
        out: &mut Img,
        ranked: Option<(&[u32], usize, (i32, i32))>,
    ) {
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
        let mut best_rank = if ranked.is_some() { vec![0u32; cells] } else { Vec::new() };
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
                    if let Some((ranks, stride, (ox, oy))) = ranked {
                        let value = ranks[(y + oy) as usize * stride + (x + ox) as usize];
                        if best_scene[i].is_none() || value > best_rank[i] {
                            best_scene[i] = Some(c);
                            best_rank[i] = value;
                        }
                    } else if best_scene[i].is_none_or(|e| rank(c) > rank(e)) {
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
pub fn frame_for_live(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    during_live(||frame_for(run,w,h,focus))
}
pub(super) fn during_live<T>(paint:impl FnOnce()->T)->T {
    struct Arm(bool);
    impl Drop for Arm {fn drop(&mut self) {LIVE_FRAME.with(|flag|flag.set(self.0));}}
    let _arm=Arm(LIVE_FRAME.with(|flag|flag.replace(true)));
    paint()
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
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    baking: Option<u64>,
    /// A torch step being drawn a stage a frame (a browser has no baker).
    #[cfg(target_arch = "wasm32")]
    staged: Option<Staged>,
}

/// A torch step's scenery part drawn, from the run as it stood when the
/// step came.
#[cfg(target_arch = "wasm32")]
struct Staged {
    flicker: u64,
    run: Box<Run>,
    bake: DungeonBake,
}

static LIVE_SCENERY: OnceLock<std::sync::Mutex<Option<LiveScenery>>> = OnceLock::new();
static SCENERY_DRAW: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static BAKE_TX: OnceLock<std::sync::mpsc::Sender<(u64, SteadyKey, u64, Run)>> = OnceLock::new();

fn scenery_draw_lock() -> std::sync::MutexGuard<'static, ()> {
    SCENERY_DRAW
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

/// The baker beside the painter. A wasm build has no threads: there a
/// torch step is drawn at once, like a room change.
#[cfg(not(target_arch = "wasm32"))]
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
/// bake the new step beside the painter: on a thread natively, a stage a
/// frame in a browser (a dungeon room's; a room drawn elsewhere is drawn
/// at once there). A room change is still drawn now.
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
            #[cfg(not(target_arch = "wasm32"))]
            Some(live) if live.steady == steady => {
                let stale = std::sync::Arc::clone(&live.img);
                if live.baking != Some(live.generation) {
                    live.baking = Some(live.generation);
                    let job = (live.generation, steady, flicker, run.clone());
                    let _ = bake_sender().send(job);
                }
                return stale;
            }
            #[cfg(target_arch = "wasm32")]
            Some(live) if live.steady == steady && !drawn_elsewhere(run) => {
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
    let img = std::sync::Arc::new(scenery(run));
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
    crate::rules::barony::print(&run.home.barony).hash(&mut h);
    run.home.trophies.hash(&mut h);
    run.home.countryside.built.hash(&mut h);
    run.home.countryside.lamps.hash(&mut h);
    (run.home.countryside.harbour == 3).hash(&mut h);
    // The tavern's bar is kept once Maud is home.
    run.home.residents.hash(&mut h);
    run.kingdom.return_to.as_ref().map(|r| &r.name).hash(&mut h);
    h.finish()
}

/// Paint the room's scenery and light it for dusk.
/// How far floor pixel `(x, y)` lies from the nearest wall straight left,
/// right, up or down, up to a tile: the floor is dressed along the walls
/// and wears away into the dark. Walls are whole tiles and a floor pixel's
/// own tile is no wall, so within a tile's reach only the next tile each
/// way can be one: the first wall pixel that way is where that tile starts.
/// (It was a walk of up to fifteen pixels four ways for every floor pixel,
/// most of the time a torch step took to draw.)
fn near_wall(room: &Room, x: i32, y: i32) -> i32 {
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

fn scenery(run: &Run) -> Img {
    let _draw = scenery_draw_lock();
    if let Some(image) = scenery_elsewhere(run) {
        return image;
    }
    let mut bake = DungeonBake::new(run);
    while !bake.step(run) {}
    bake.cv
}

/// Whether `run`'s room is drawn whole by its own module: the Wanted hall,
/// the side-on halls and every room at home. A dungeon room is drawn here,
/// in stages (`DungeonBake`).
fn drawn_elsewhere(run: &Run) -> bool {
    run.room().kind == RoomKind::Wanted || run.side_on() || run.at_home_now()
}

/// The scenery of a room drawn elsewhere (`drawn_elsewhere`), or `None`.
fn scenery_elsewhere(run: &Run) -> Option<Img> {
    if !drawn_elsewhere(run) {
        return None;
    }
    if run.room().kind == RoomKind::Wanted {
        return Some(wanted::scenery(run));
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
            RoomKind::Hall | RoomKind::Stockpile | RoomKind::Workshop | RoomKind::Quarters => {
                home::settlement_scenery(run)
            }
            kind if crate::rules::college::is_room(kind) => college::scenery(run),
            kind if crate::rules::ranch::is_room(kind) => ranch::scenery(run),
            kind if kind.in_world() => world::scenery(run),
            _ => home::scenery(run),
        };
        home::research_exhibits(&mut image, run);
        return Some(image);
    }
    None
}

/// A dungeon room's lit scenery, drawn in stages: the floor's marks, the
/// walls, the fixtures, the material relief, the light. Drawn all at once
/// it is `scenery`. A browser, which has no thread to draw a torch step
/// beside the painter as a native host does, draws it a stage a frame
/// while the step before shows (`scenery_live`). The stages split the same
/// loops at row boundaries, in the same order: the pixels are the same.
struct DungeonBake {
    cv: Img,
    lights: Vec<Light>,
    stage: u8,
}

impl DungeonBake {
    /// The floor in two halves, the walls in two, the fixtures, the
    /// relief, the light in two: none of them more than a millisecond or
    /// so of a desktop's time.
    const STAGES: u8 = 8;

    fn new(run: &Run) -> DungeonBake {
        let (pw, ph) = room_px(run.room());
        DungeonBake { cv: Img::black(pw, ph), lights: Vec::new(), stage: 0 }
    }

    /// Draw the next stage of `run`'s scenery; true once it is whole.
    fn step(&mut self, run: &Run) -> bool {
        let (h, rows) = (self.cv.h, run.room().rows as i32);
        match self.stage {
            0 => floor_marks(run, &mut self.cv, 0..h / 2),
            1 => floor_marks(run, &mut self.cv, h / 2..h),
            2 => wall_tiles(run, &mut self.cv, &mut self.lights, 0..rows / 2),
            3 => wall_tiles(run, &mut self.cv, &mut self.lights, rows / 2..rows),
            4 => room_fixtures(run, &mut self.cv, &mut self.lights),
            5 => super::material::relief(&mut self.cv),
            6 => light_rows(&mut self.cv, &self.lights, style(run.dungeon.pack).pool, 0.0, &[], 0..h / 2),
            _ => light_rows(&mut self.cv, &self.lights, style(run.dungeon.pack).pool, 0.0, &[], h / 2..h),
        }
        self.stage += 1;
        self.stage >= Self::STAGES
    }
}

/// The floor's dressing, pixel rows `rows`: skirting along the walls, the
/// pack's marks, hazards.
fn floor_marks(run: &Run, cv: &mut Img, rows: std::ops::Range<i32>) {
    let room = run.room();
    let pack = run.dungeon.pack;
    let t = (run.tick as u32 / 3) as i32;
    let pw = cv.w;
    for y in rows {
        for x in 0..pw {
            let ink = match room.tile(x / TILE, y / TILE) {
                Tile::Floor => super::material::skirting(pack as u32, x, y, near_wall(room, x, y)).or_else(|| floor_mark(pack, x, y)),
                Tile::Door => floor_mark(pack, x, y),
                Tile::Hazard => hazard_mark(pack, room, x, y, t),
                _ => None,
            };
            if let Some(ch) = ink {
                cv.put(x, y, ch);
            }
        }
    }
}

/// The walls, blocks and corner braziers of tile rows `rows`, and the
/// braziers' and hot floors' lights.
fn wall_tiles(run: &Run, cv: &mut Img, lights: &mut Vec<Light>, rows: std::ops::Range<i32>) {
    let room = run.room();
    let pack = run.dungeon.pack;
    let st = style(pack);
    let tick = run.tick as u32;
    let fighting = run.phase == Phase::Fighting;
    let tiles = Tiles::get();
    for row in rows {
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
                Tile::Wall => {
                    let floor = |c: i32, r: i32| matches!(room.tile(c, r), Tile::Floor | Tile::Door | Tile::Stairs | Tile::Hazard);
                    let open = [floor(col, row - 1), floor(col + 1, row), floor(col, row + 1), floor(col - 1, row)];
                    let (ramp, rough): (&[char], bool) = match pack {
                        Pack::Crypt => (super::material::ramp::SLATE, false),
                        Pack::Archive => (super::material::ramp::STONE, false),
                        Pack::Cavern => (&['n', 'b', 'I', 'B', 'P', 'r', 'o'], true),
                        Pack::Fungal => (&['n', 'D', 'F', 'E', 'e', 'l', 'm'], true),
                        Pack::Hellforge | Pack::Unknown => (&['k', 'K', 'X', 'g', 'j', 'G', 'J'], pack == Pack::Unknown),
                    };
                    super::material::wall_tile(cv, (x, y), TILE, open, ramp, rough, 300 + pack as u32);
                }
                // A forge-hall's furnace and anvils are drawn whole below.
                Tile::Block if room.kind == RoomKind::Forge => {}
                // A nook's furniture is drawn whole below.
                Tile::Block if room.kind.nook() => {}
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
}

/// What stands in the room over its floor and walls: the doorways' torches
/// and bars, the stairs, altars, furnaces and nooks, the chest, the traps
/// at rest, a cracked wall; and their lights.
fn room_fixtures(run: &Run, cv: &mut Img, lights: &mut Vec<Light>) {
    let room = run.room();
    let tick = run.tick as u32;
    let barred = run.barred();
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
            if first {
                cv.stamp(&sprites::stairs(!barred), col * TILE, row * TILE);
            }
        }
    }
    if room.kind == crate::rules::RoomKind::Sanctuary {
        // The reforge altar: Tobbin's travelling anvil on a stone plinth,
        // candles either side, its coals always lit.
        let altar = sprites::dais(tick, run.wishing);
        let (ax, ay) = room.altar();
        let (x, y) = at(ax, ay);
        cv.stamp(&altar, x - altar.w / 2, y - altar.h / 2);
        lights.push(fire(x, y - 6, if run.wishing { 84.0 } else { 64.0 }, 0.55));
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
        secrets::crack(cv, (cx, cy), side);
    }
    if room.kind == RoomKind::Secret {
        // The vault's gold catches what light there is.
        let (x, y) = at(room.width() / 2.0, room.height() / 2.0);
        lights.push(fire(x, y, 70.0, 0.45));
    }
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
fn dusk(cv: &mut Img, lights: &[Light], pool: (char, char)) {
    dusk_with(cv, lights, pool, 0.0);
}

/// Dusk with the value hierarchy of the world above: a solid mass (a tree,
/// a wall, a roof, a prop) is lifted `lift` steps toward the light, while
/// the sparse marks of the ground keep the full dusk. Ground stays dark and
/// calm; what stands on it reads first.
fn dusk_with(cv: &mut Img, lights: &[Light], pool: (char, char), lift: f32) {
    // Flat fills take the shared material finish before the light falls.
    super::material::relief(cv);
    let solid: Vec<bool> = if lift > 0.0 {
        (0..cv.h * cv.w)
            .map(|i| {
                let (x, y) = (i % cv.w, i / cv.w);
                let inked = |dx: i32, dy: i32| cv.get(x + dx, y + dy).is_some_and(|c| c != BLACK);
                inked(0, 0)
                    && [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (1, -1), (-1, 1)]
                        .iter()
                        .filter(|&&(dx, dy)| inked(dx, dy))
                        .count()
                        >= 6
            })
            .collect()
    } else {
        Vec::new()
    };
    let h = cv.h;
    light_rows(cv, lights, pool, lift, &solid, 0..h);
}

/// The light falling on pixel rows `rows` of a scene its relief is on
/// (`dusk_with`): each pixel stepped down its bank by how dark it lies,
/// warmed where fire reaches it. `solid` marks the masses `lift` raises.
fn light_rows(cv: &mut Img, lights: &[Light], (cool, lit): (char, char), lift: f32, solid: &[bool], rows: std::ops::Range<i32>) {
    // The dapple and the firelit inks never change: work them out once.
    static DAPPLE: OnceLock<Vec<f32>> = OnceLock::new();
    static WARM: OnceLock<super::ink::RgbMap<Rgb>> = OnceLock::new();
    let dapple = DAPPLE.get_or_init(|| {
        (0..NATIVE_W * NATIVE_H)
            .map(|i| {
                let (x, y) = ((i % NATIVE_W) as f32 + 0.5, (i / NATIVE_W) as f32 + 0.5);
                (vnoise(x / 48.0, y / 48.0, 77) - 0.5) * 0.14
            })
            .collect()
    });
    let warmed = WARM.get_or_init(|| PALETTE.iter().map(|&v| (rgb(v), warm(rgb(v)))).collect());
    for y in rows {
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
                    // Dither only across the band between two steps: a calm
                    // field holds one step, so material texture is not lost
                    // under a checkerboard.
                    let mass = lift > 0.0 && solid[(y * cv.w + x) as usize];
                    let v = ((1.0 - level) * STEPS - if mass { lift } else { 0.0 }).max(0.0);
                    let t = ((v.fract() - 0.5) * 3.5 + 0.5).clamp(0.0, 1.0);
                    let steps = (v.floor() + if t > bayer(x, y) { 1.0 } else { 0.0 }).max(0.0) as usize;
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
    use crate::rules::hazards::{self as hz, Spikes};
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

/// A monster or boss as the room draws it this tick, at the size it
/// stands (`stature`).
fn foe_figure(run: &Run, enemy: &crate::rules::Enemy, tick: u32) -> Img {
    if let Some(boss) = enemy.boss.and_then(|i| run.bosses.get(usize::from(i))) {
        if let Some(im) = signatures::figure(run, enemy, boss, tick) {
            return im;
        }
    }
    if enemy.kind == EnemyKind::Spiderling
        && enemy.stage == crate::rules::signatures::TENDER
    {
        return signatures::tender(tick);
    }
    if let Some(im) = enemy.boss.and_then(|i| stature::boss_figure(run, i)) {
        return im;
    }
    let (im, k) = match enemy.kind {
        // A loot goblin bolts for the door with its sack over its back.
        EnemyKind::Goblin => {
            let im = sprites::goblin(enemy.burden(), enemy.fleeing(), tick);
            (if enemy.dir.0 < 0.0 { im.flip_h() } else { im }, 1.0)
        }
        // The Broodmarrow: its seams tell when it means to throw.
        EnemyKind::Broodmarrow => (sprites::broodmarrow(enemy.stage == 1, tick), 1.0),
        // The Oathbound: the visor burns red only when the oath is broken.
        EnemyKind::Oathbound => (sprites::oathbound(enemy.stage != 0, tick), 1.0),
        // Humphrey: his stage is his pose, his heading is his face.
        EnemyKind::Risen => {
            let pose = if enemy.hurt > 0 {
                sprites::RisenPose::Hit
            } else {
                match enemy.stage {
                    1 => sprites::RisenPose::WindUp,
                    2 => sprites::RisenPose::Shove,
                    3 => sprites::RisenPose::Rest,
                    // Mid-shamble he now and then simply stops, and stares.
                    _ if enemy.timer % 60 < 8 => sprites::RisenPose::Idle,
                    _ => sprites::RisenPose::Walk,
                }
            };
            let (dir, flip) = if enemy.dir.0.abs() > enemy.dir.1.abs() {
                (sprites::RisenDir::East, enemy.dir.0 < 0.0)
            } else if enemy.dir.1 > 0.0 {
                (sprites::RisenDir::South, false)
            } else {
                (sprites::RisenDir::North, false)
            };
            let frame = u32::from((tick / 14).is_multiple_of(2));
            let im = sprites::risen(pose, dir, frame);
            (if flip { im.flip_h() } else { im }, 1.0)
        }
        // A slime's size is its stage; a giant is drawn as big as it is.
        EnemyKind::Slime => (
            sprites::enemy(enemy.kind, tick),
            enemy.radius() / sprites::SLIME_REACH,
        ),
        kind if kind.critter() => (critters::art(run, enemy, tick), 1.0),
        kind if enemy.radius() > kind.radius() * 1.1 => {
            (sprites::enemy(kind, tick), enemy.radius() / kind.radius())
        }
        kind => (sprites::enemy(kind, tick), 1.0),
    };
    // Every creature stands in the same light as the room.
    let im = super::material::shade_sprite(&im);
    if enemy.is_boss() {
        // The Pit's beast and the lair's dragon: bosses without a file.
        let im = if k > 1.0 { enlarge(&im, k) } else { im };
        stature::boss(&im, enemy.radius(), stature::floor(run, enemy))
    } else {
        stature::monster(enemy.kind, im, k)
    }
}

fn figures(cv: &mut Img, run: &Run) {
    figures_with(cv, run, false);
}

fn figures_with(cv: &mut Img, run: &Run, modern: bool) {
    let tick = run.tick as u32;
    let room = run.room();
    // Side-on, a body's position is its middle: figures stand on its feet.
    let feet = if run.side_on() { FOOT } else { 0.0 };
    for item in &room.items {
        let (x, y) = at(item.x, item.y);
        let im = cards::floor_card(run.book.get(&item.card), tick);
        cv.stamp(&im, x - im.w / 2, y - im.h / 2);
    }
    hand::over(cv, run);
    runes::floor(cv, run);
    secrets::draught(cv, run);
    secrets::snibbet(cv, run);
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.hp == 0) {
        // A fallen knight is a skeleton that has come apart where they stood.
        let (x, y) = at(hero.x, hero.y + feet);
        bones::draw(cv, hero, id, run.tick, run.phase == Phase::Wiped, x, y);
    }
    hireling::delve(cv, run);
    frogs::wild(cv, run);
    frogs::shown(cv, run);
    // The crowd, painted in three ranks: guardians first (their own parts
    // and escorts stand over them), then the other monsters, then the
    // knights, who are never lost behind a monster. Within a rank whoever
    // stands lower in the room is in front.
    let mut crowd: Vec<(f32, u8, Box<dyn FnOnce(&mut Img) + '_>)> = Vec::new();
    for enemy in &run.enemies {
        let rank = u8::from(enemy.boss.is_none());
        crowd.push((enemy.y, rank, Box::new(move |cv: &mut Img| {
            let mut im = foe_figure(run, enemy, tick);
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
            critters::marks(cv, run, enemy, tick);
            let mut im = marks::flashed(&im, enemy, run.tick);
            let foot_lift = signatures::foot_lift(run, enemy);
            let (x, y) = if !run.side_on() {
                at(enemy.x, enemy.y + foot_lift)
            } else if enemy.kind.flies() {
                // A flier hangs on its middle; a walker stands on its feet.
                let (x, y) = at(enemy.x, enemy.y);
                (x, y + im.h / 2)
            } else if enemy.boss.is_some() {
                // A boss side-on stands with its feet a radius below its middle.
                at(enemy.x, enemy.y + enemy.radius())
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
                        let d = |h: &&crate::rules::Hero| {
                            (h.x - enemy.x).hypot(h.y - enemy.y)
                        };
                        d(a).total_cmp(&d(b))
                    })
                    .is_some_and(|h| h.x < enemy.x);
                if left {
                    im = im.flip_h();
                }
                cv.stamp(&im, x - im.w / 2, y - im.h * 2 / 3);
                marks::enemy_bar(cv, enemy, x, y - im.h * 2 / 3 + im.h, &im);
                marks::last_stand(cv, run, enemy, tick);
                return;
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
            if !(modern && art_v2::foe_supported(run, enemy)) {
                if live_frame() && !run.side_on() {
                    stand_at(cv, &im, enemy.x, enemy.y + foot_lift, 0);
                } else {
                    stand(cv, &im, x, y);
                }
            }
            marks::enemy_bar(cv, enemy, x, y, &im);
            marks::last_stand(cv, run, enemy, tick);
        })));
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
    pitstop::booth(cv, run);
    // A knight in the saddle at the lists is drawn with their horse.
    let riding: Vec<u32> = run.joust.as_ref().map_or_else(Vec::new, |j| {
        std::iter::once(j.knight).chain(j.foe.as_ref().map(|f| f.id)).collect()
    });
    for (&id, hero) in run
        .players
        .iter()
        .filter(|(id, h)| h.hp > 0 && h.privy == 0 && h.soak == 0 && !riding.contains(id))
    {
        crowd.push((hero.y, 2, Box::new(move |cv: &mut Img| {
            let (x, y) = at(hero.x, hero.y + feet);
            // A card's aura glows on the ground under the knight.
            if let Some(aura) = hero.newest(&run.book, looks::Slot::Aura) {
                let glow = knights::frame_img(aura.frame(run.tick));
                cv.stamp(&glow, x - glow.w / 2, y - glow.h / 2);
            }
            let im = if let Some(avatar) = &hero.avatar {
                knights::avatar_figure(hero, &avatar.pixels, run.shield_out(id))
            } else if hero.pit.stop.is_some() {
                knights::pit_figure(hero, id, tick)
            } else {
                // The knight at the stage of dress their health leaves them,
                // with the shield they hold up turned to the aim.
                knights::figure_with(hero, id, run.shield_out(id))
            };
            // What their cards put on them: a bigger shield, spikes, a helm.
            let mut im = knights::dress(&im, &hero.worn(&run.book), run.tick);
            if hero.frog() {
                // Hexed: a frog, for now.
                im = sprites::frog(tick);
            }
            if hero.chilled > 0 {
                // Chilled by the Lich: rimed over, like a frozen monster.
                im = ults::iced(&im);
            }
            if run.works.kits.get(&id).is_some_and(|k| k.duck) {
                // Down on one knee: a head lower, shots go over.
                im = works::crouched(&im);
            }
            if hero.aim_x < -0.3 {
                im = im.flip_h();
            }
            if hero.has_rune(crate::rules::runes::RuneKind::Invisibility) {
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
                return;
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
                return;
            }
            if hero.tossed > 0 {
                // Thrown by a Ravage: up on an arc, tumbling, and back down.
                use crate::rules::tide::TOSSED;
                let flight = (TOSSED - hero.tossed.min(TOSSED)) as f32 / TOSSED as f32;
                let lift = ((flight * std::f32::consts::PI).sin() * 10.0) as i32;
                let mut turned = im.clone();
                for _ in 0..(hero.tossed / 3) % 4 {
                    turned = sprites::quarter_turn(&turned);
                }
                stand_at(cv, &turned, hero.x, hero.y + feet, -lift);
                return;
            }
            // Blink while a hit's grace lasts.
            let blink = hero.invulnerable > 0
                && hero.invulnerable < run.tune.hit_ward_ticks().max(1)
                && (tick / 2).is_multiple_of(2)
                && !strike::whiteout(hero);
            if modern && art_v2::knight_supported(run, hero, id) {
                frogs::shoulder(cv, hero, x, y, tick);
            } else if hero.struck > 0 {
                // Just hit: white, then thrown back and bent over.
                let (posed, dx, dy) = strike::pose(&im, hero);
                stand(cv, &posed, x + dx, y + dy);
                frogs::shoulder(cv, hero, x + dx, y + dy, tick);
            } else if !blink {
                stand_at(cv, &im, hero.x, hero.y + feet, -pitstop::bob(hero, tick));
                frogs::shoulder(cv, hero, x, y, tick);
            }
            strike::sparks(cv, hero, id, x, y);
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
            if hero.spin > 0 {
                // The dwarf's axes, round her middle.
                knights::spin_marks(
                    cv,
                    x,
                    y - 7,
                    hero.spin,
                    crate::rules::spin::REACH * UNIT,
                );
            }
            // The helmet and tabard that health has cost, in the air.
            knights::flung(cv, hero, x, y, if hero.aim_x < 0.0 { 1 } else { -1 });
            marks::hero_bar(cv, hero, x, y);
            // The knight's true hit point: one bright pixel.
            let (hx, hy) = at(hero.x, hero.y);
            cv.put(hx, hy, 'w');
            if hero.unarmed() && hero.swing > 0 {
                let reach = 0.6 + hero.swing as f32 * 0.13;
                let (fx, fy) = at(hero.x + hero.aim_x * reach, hero.y + hero.aim_y * reach);
                cv.rect(fx - 2, fy - 10, 4, 4, 't');
                cv.put(fx + 1, fy - 10, 'O');
            }
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
        })));
    }
    crowd.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.total_cmp(&b.0)));
    for (_, _, paint) in crowd {
        paint(cv);
    }
    runes::auras(cv, run);
    // Shots are drawn at twice their sprite size: a hairline reads as noise
    // at speed, a two-pixel stroke reads as an arrow.
    let mut mark = Img::new(25, 25);
    for shot in &run.projectiles {
        // Beams, marks and mines first; a beam or a waiting shot is done.
        if volleys::draw(cv, run, shot) {
            continue;
        }
        if modern && art_v2::shot_supported(shot) {continue;}
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
        if let Some(sprite) = shot
            .sprite_card()
            .and_then(|id| run.book.get(id))
            .and_then(|card| card.looks.shot.as_ref())
        {
            // A card's own shot, turned the way it flies.
            let mut im = knights::frame_img(sprite.frame(run.tick));
            if shot.vx < 0.0 {
                im = im.flip_h();
            }
            mark.stamp(&im, 12 - im.w / 2, 12 - im.h / 2);
        } else if shot.sprite_card().is_some() {
            sprites::shot(&mut mark, (12, 12), (shot.vx, shot.vy), shot.kind);
        } else if let Some((colour, _)) = &shot.look
            && class_looks::shot(&mut mark, (12, 12), (shot.vx, shot.vy), colour, run.tick)
        {
            // A class's own shot: the Mage's arcane bolt, the Healer's spark.
        } else if let Some((colour, shape)) = &shot.look {
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
    use crate::rules::{BLAST_TICKS, HZ};
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
            crate::rules::ledge::FOOT
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

/// Each knight's morningstars and the chains that hold them, or what a
/// card of theirs spins round them instead (`sprite orbit`), unchained.
fn orbits(cv: &mut Img, run: &Run) {
    let ball = sprites::morningstar();
    for (&id, hero) in run.players.iter().filter(|(_, h)| h.privy == 0) {
        let own = hero
            .newest(&run.book, looks::Slot::Orbit)
            .map(|s| knights::frame_img(s.frame(run.tick)));
        for (bx, by) in run.orbits(id) {
            let (hx, hy) = at(hero.x, hero.y - 0.6);
            let (x, y) = at(bx, by);
            if let Some(im) = &own {
                cv.stamp(im, x - im.w / 2, y - im.h / 2);
                continue;
            }
            for k in 1..6 {
                cv.put(hx + (x - hx) * k / 6, hy + (y - hy) * k / 6, 'G');
            }
            cv.stamp(&ball, x - ball.w / 2, y - ball.h / 2);
        }
    }
}

/// The creatures cards summoned, in their card's look (`sprite summon`),
/// each a few frames out of step with its fellows.
fn summoned(cv: &mut Img, run: &Run) {
    for imp in &run.imps {
        let tick = run.tick + u64::from(imp.seat) * 3;
        let mut im = run
            .book
            .get(&imp.card)
            .and_then(|c| c.looks.summon.as_ref())
            .map_or_else(
                || knights::imp_default(imp.behaviour, tick),
                |s| knights::frame_img(s.frame(tick)),
            );
        if imp.left_facing {
            im = im.flip_h();
        }
        let (x, y) = at(imp.x, imp.y);
        // A turret stands; the rest fly a little off the floor.
        let lift = if imp.behaviour == imps::Behaviour::Turret { 0 } else { 3 };
        for dx in -1..=1 {
            cv.put(x + dx, y + 1, 'K');
        }
        cv.stamp(&im, x - im.w / 2, y - im.h + 1 - lift);
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
    let y = (NATIVE_H - sign.h) / 2;
    cv.stamp(&sign, (NATIVE_W - sign.w) / 2, y);
    // And the way on (`rules::again`): Enter asks, and once a knight has,
    // how long until the party goes.
    let hint = match &run.again {
        None => "ENTER: BACK TO THE KINGDOM".to_string(),
        Some(again) => {
            let secs = again.left.div_ceil(crate::rules::HZ).max(1);
            format!("BACK TO THE KINGDOM IN {secs}  ENTER: NOW")
        }
    };
    let w = super::ink::text_width(&hint);
    let mut plate = Img::new(w + 6, 11);
    plate.rect(0, 0, w + 6, 11, 'K');
    plate.text(3, 2, &hint, '6');
    cv.stamp(&plate, (NATIVE_W - plate.w) / 2, y + sign.h + 2);
}

/// For a few seconds after a Sanctuary opens: its name, and what it is for.
fn sanctuary_sign(cv: &mut Img, run: &Run) {
    let hz = u64::from(crate::rules::HZ);
    // A room won opens a wish window: a short sign says so.
    let in_sanctuary = run.room().kind == crate::rules::RoomKind::Sanctuary;
    let (at, title, hint, last) = match (run.hallowed, run.window.as_ref()) {
        // Only in the Sanctuary itself: a knight who walks straight out
        // leaves its sign behind.
        (Some(at), _) if in_sanctuary && run.tick.saturating_sub(at) < 4 * hz => {
            (at, "SANCTUARY", "T: ONE WISH", 4)
        }
        // A folly has the screen: its title card, not the wish sign.
        (_, Some(w))
            if run.room().kind != crate::rules::RoomKind::Sanctuary && !run.folly_running() =>
        {
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
pub fn camera(run: &Run, focus: Option<u32>) -> (i32, i32) {
    let (pw, ph) = room_px(run.room());
    if run.room().kind == RoomKind::Kingdom {
        let hero = focus
            .and_then(|id| run.players.get(&id))
            .or_else(|| run.players.get(&1))
            // A host who plays no knight sends none (`Hero::absent`): the
            // party's first.
            .or_else(|| run.players.values().next());
        let (cx, cy) = hero
            .map(|h| h.camera.unwrap_or((h.x, h.y - 1.0)))
            .unwrap_or((WIDTH / 2.0, HEIGHT / 2.0));
        let (cx, cy) = at(cx, cy);
        return (
            (cx - NATIVE_W / 2).clamp(0, pw - NATIVE_W),
            (cy - NATIVE_H / 2).clamp(0, ph - NATIVE_H),
        );
    }
    let alive = |h: &&crate::rules::Hero| h.hp > 0;
    let watched: Vec<(f32, f32)> = match focus.and_then(|id| run.players.get(&id)).filter(alive) {
        Some(hero) => vec![(hero.x, hero.y)],
        None => run
            .players
            .values()
            .filter(alive)
            .map(|h| (h.x, h.y))
            .collect(),
    };
    // In the Wanted hall the view takes in the outlaw too.
    let mut watched = watched;
    if run.room().kind == RoomKind::Wanted
        && let Some(e) = run.enemies.iter().find(|e| e.boss.is_some() && e.hp > 0)
    {
        watched.push((e.x, e.y));
    } else if run.room().kind == RoomKind::Wanted
        && let Some(fall) = run.works.outlaw.fall
        && run.tick.saturating_sub(fall.at)
            < u64::from(crate::rules::outlaw::FALL)
    {
        // The view stays on the outlaw while it falls.
        watched.push((fall.x, fall.y));
    }
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

/// One view of `full` from `(ox, oy)`.
fn crop(full: &Img, (ox, oy): (i32, i32)) -> Img {
    Img::from_view(full, ox, oy, NATIVE_W, NATIVE_H)
}

/// What the view cannot show: a mark on its edge toward each monster (red)
/// and each knight (their colours) out of sight in a great hall. The mark is
/// an arrowhead pointing out, so it reads as a pointer and never as a small
/// monster at the edge of the room.
/// A gold arrow at `(ax, ay)` with its tip toward `(ux, uy)`, outlined so it
/// reads on any ground, and its two- or three-word label just above or
/// below it (`edge`: pinned to the view's edge, so the label goes toward
/// the middle).
pub(super) fn pointer(
    over: &mut Img,
    (ax, ay): (f32, f32),
    (ux, uy): (f32, f32),
    name: &str,
    edge: bool,
) -> Vec<(i32, i32, i32, i32)> {
    for (ink, grow) in [('K', 1.5), ('6', 0.0)] {
        for py in -10..=10 {
            for px in -10..=10 {
                let along = px as f32 * ux + py as f32 * uy;
                let across = (py as f32 * ux - px as f32 * uy).abs();
                if (-5.0 - grow..=7.0 + grow).contains(&along)
                    && across <= (7.0 + grow - along) * 0.5 + grow * 0.5
                {
                    over.put(ax.round() as i32 + px, ay.round() as i32 + py, ink);
                }
            }
        }
    }
    let label = name.to_uppercase();
    let w = super::ink::text_width(&label);
    let lx = (ax.round() as i32 - w / 2).clamp(4, NATIVE_W - w - 4);
    let ly = if edge && ay < (NATIVE_H / 2) as f32 || !edge && uy < -0.5 {
        ay.round() as i32 + 13
    } else {
        ay.round() as i32 - 21
    }
    .clamp(4, NATIVE_H - 13);
    over.rect(lx - 2, ly - 2, w + 4, 11, 'K');
    over.text(lx, ly, &label, '6');
    vec![(lx - 2, ly - 2, w + 4, 11), (ax as i32 - 10, ay as i32 - 10, 21, 21)]
}

/// The house rule for "press E here": the key and a word or two, shown only
/// while a knight stands within reach of the spot (pixels `(x, y)`).
pub(super) fn near_prompt(cv: &mut Img, run: &Run, (x, y): (i32, i32), label: &str) {
    let reach = 2.5 * TILE as f32;
    let near = run.players.values().any(|h| {
        let (hx, hy) = at(h.x, h.y);
        h.hp > 0 && ((hx - x) as f32).hypot((hy - y) as f32) < reach
    });
    if !near {
        return;
    }
    let label = format!("E: {label}");
    let w = super::ink::text_width(&label);
    let (lx, ly) = ((x - w / 2).clamp(2, cv.w - w - 2), (y - 30).max(2));
    cv.rect(lx - 2, ly - 1, w + 4, 10, 'K');
    cv.text(lx, ly, &label, '6');
}

/// A way that is shut: two planks nailed across it.
pub(super) fn shut_planks(cv: &mut Img, x: i32, y: i32, w: i32) {
    cv.line(x + 1, y + 1, x + w - 2, y + TILE - 3, 'B');
    cv.line(x + 1, y + TILE - 3, x + w - 2, y + 1, 'B');
    cv.line(x + 2, y + 1, x + w - 1, y + TILE - 3, 'r');
    cv.line(x + 2, y + TILE - 3, x + w - 1, y + 1, 'r');
}

/// A way that is open: a small arrow up the stair.
pub(super) fn open_arrow(cv: &mut Img, x: i32, y: i32) {
    for k in 0..4 {
        cv.line(x - k, y + k, x + k, y + k, '9');
    }
    cv.line(x, y + 4, x, y + 8, '9');
}

/// A Sanctuary's field banner: the next class's colour on a pole, and the
/// key and price when the knight is close.
fn field_banner(over: &mut Img, run: &Run, (ox, oy): (i32, i32), focus: Option<u32>) {
    use crate::rules::classes::{self, FIELD_PRICE};
    let Some((bx, by)) = run.room().field_banner() else {
        return;
    };
    let Some(hero) = run.players.get(&focus.unwrap_or(1)) else {
        return;
    };
    let next = classes::next_class(hero);
    let (x, y) = at(bx, by);
    let (x, y) = (x - ox, y - oy);
    over.rect(x - 4, y - 3, 9, 3, 'G');
    over.rect(x - 1, y - 30, 2, 28, 'h');
    let flap = i32::from(run.tick / 8 % 2 == 0);
    over.rect(x + 1, y - 29 + flap, 13, 11, next.ink);
    over.rect(x + 1, y - 29 + flap, 13, 1, 'K');
    if (hero.x - bx).hypot(hero.y - by) < 3.0 * TILE_UNITS {
        let label = format!("E: {} {FIELD_PRICE}G", next.name.to_uppercase());
        let w = super::ink::text_width(&label);
        let (lx, ly) = ((x - w / 2).clamp(2, NATIVE_W - w - 2), y + 2);
        over.rect(lx - 2, ly - 1, w + 4, 10, 'K');
        over.text(lx, ly, &label, '6');
    }
}

/// The Wax Maze: wax on every corridor, holy water in the corners, the
/// four wraiths (pale and fleeing after holy water), the title card, and
/// the standings in wax.
fn folly_layer(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    use crate::rules::folly::Temper;
    let Some(maze) = run.folly.as_ref().filter(|m| m.room == run.at) else {
        return;
    };
    let cols = run.room().cols as i32;
    let done = maze.done;
    let place = |i: u16| {
        let (tx, ty) = (i as i32 % cols, i as i32 / cols);
        let (x, y) = at((tx as f32 + 0.5) * TILE_UNITS, (ty as f32 + 0.5) * TILE_UNITS);
        (x - ox, y - oy)
    };
    // Once the podium is called, the maze is just a crypt again.
    for &i in maze.wax.iter().filter(|_| !done) {
        let (x, y) = place(i);
        over.rect(x - 1, y - 1, 3, 3, 'T');
        over.put(x - 1, y - 1, '9');
    }
    for &i in maze.holy.iter().filter(|_| !done) {
        let (x, y) = place(i);
        let bob = [0, 1, 1, 0][(run.tick / 8 % 4) as usize];
        over.rect(x - 1, y - 7 - bob, 3, 2, 'h');
        over.rect(x - 3, y - 5 - bob, 7, 7, 'z');
        over.rect(x - 2, y - 4 - bob, 2, 3, 'w');
    }
    let pale = maze.pale > 0;
    let flash = pale && maze.pale < 2 * crate::rules::HZ && run.tick / 4 % 2 == 0;
    let ghost = [
        "...xxxxxx...",
        "..xxxxxxxx..",
        ".xxxxxxxxxx.",
        ".xxwwxxwwxx.",
        ".xxwkxxwkxx.",
        ".xxxxxxxxxx.",
        ".xxxxxxxxxx.",
        ".xxxxxxxxxx.",
        ".xxxxxxxxxx.",
        ".xx.xxx.xxx.",
        ".x...x...xx.",
    ];
    for w in maze.wraiths.iter().filter(|w| !done && (w.rest == 0 || run.tick / 6 % 2 == 0)) {
        let body = if flash {
            'W'
        } else if pale {
            'Q'
        } else {
            match w.temper {
                Temper::Hound => '7',
                Temper::Cutter => 'c',
                Temper::Flanker => '2',
                Temper::Drifter => '4',
            }
        };
        let (tx, ty) = w.pos(pale);
        let (x, y) = at(tx * TILE_UNITS, ty * TILE_UNITS);
        let (x, y) = (x - ox - 6, y - oy - 8 + [0, 1, 0, -1][(run.tick / 5 % 4) as usize]);
        for (ry, row) in ghost.iter().enumerate() {
            for (rx, c) in row.chars().enumerate() {
                let ink = match c {
                    'x' => body,
                    'w' => if pale { 'H' } else { 'W' },
                    'k' => if pale { 'H' } else { 'q' },
                    _ => continue,
                };
                over.put(x + rx as i32, y + ry as i32, ink);
            }
        }
    }
    // The standings in wax, top right, and the candle's seconds.
    let head = if maze.done { "WAX MAZE  DONE".to_string() } else { format!("WAX MAZE  {}S", maze.left()) };
    let rows: Vec<(u32, u32)> = maze.eaten.iter().map(|(&id, &n)| (id, u32::from(n))).collect();
    folly_board(over, run, &head, &rows);
    if maze.titled() {
        folly_title(over, "THE WAX MAZE", "EAT WAX. FLEE WRAITHS.");
    }
}

/// The Belfry Shoot's board and title card (the bats are the room's own).
fn bats_layer(over: &mut Img, run: &Run) {
    let Some(shoot) = run.bats.as_ref().filter(|b| b.room == run.at) else {
        return;
    };
    let head = if shoot.done { "BELFRY  DONE".to_string() } else { format!("BELFRY  {}S", shoot.left()) };
    let rows: Vec<(u32, u32)> = shoot.scores.iter().map(|(&id, &n)| (id, n)).collect();
    folly_board(over, run, &head, &rows);
    if shoot.titled() {
        folly_title(over, "THE BELFRY SHOOT", "SHOOT BATS.");
    }
}

/// The Keg Gantry: the ogre on the top girder, the kegs rolling and
/// falling, the board (how high each knight has climbed) and the title.
fn gantry_layer(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    use crate::rules::gantry::TOP_ROW;
    let Some(g) = run.gantry.as_ref().filter(|g| g.room == run.at) else {
        return;
    };
    if !g.done {
        // The ogre, swaying at the top-left with a keg on his shoulder.
        let (x, y) = at(1.6 * TILE_UNITS, (TOP_ROW as f32 + 0.1) * TILE_UNITS);
        let (x, y) = (x - ox, y - oy);
        let sway = [0, 1, 0, -1][(run.tick / 8 % 4) as usize];
        let ogre = [
            "....mmmm....",
            "...mmmmmm...",
            "...mkmmkm...",
            "...mmmmmm...",
            "....m77m....",
            ".mmmmmmmmmm.",
            "mmmMMMMMMmmm",
            "m.mMMMMMMm.m",
            "m.mMMMMMMm.m",
            "...MMMMMM...",
            "...mm..mm...",
            "...mm..mm...",
        ];
        // Drawn twice the size: an ogre should loom.
        for (ry, row) in ogre.iter().enumerate() {
            for (rx, c) in row.chars().enumerate() {
                if c != '.' {
                    over.rect(x - 8 + sway + rx as i32 * 2, y - 24 + ry as i32 * 2, 2, 2, c);
                }
            }
        }
        for k in &g.kegs {
            let (kx, ky) = k.pos();
            let (px, py) = at(kx * TILE_UNITS / TILE_UNITS, ky);
            let (px, py) = (px - ox, py - oy);
            let roll = (kx * 4.0) as i32 % 2 == 0;
            over.rect(px - 4, py - 4, 9, 8, 'R');
            over.rect(px - 4, py - 2, 9, 1, 'h');
            over.rect(px - 4, py + 1, 9, 1, 'h');
            over.put(px - 1 + i32::from(roll), py - 1, 'o');
        }
    }
    let head = if g.done { "GANTRY  DONE".to_string() } else { format!("GANTRY  {}S", g.left()) };
    let rows: Vec<(u32, u32)> = g.height.iter().map(|(&id, &h)| (id, u32::from(h))).collect();
    folly_board(over, run, &head, &rows);
    if g.titled() {
        folly_title(over, "THE KEG GANTRY", "CLIMB. JUMP KEGS.");
    }
}

/// The Powder Room: crates, lit kegs, the fire of a blast, the board and
/// the title.
/// Book Hop's flood, under the knights: ink in the lanes, the banks and the
/// landing dry, the books drifting, and ripples where someone sank.
fn hop_floor(cv: &mut Img, run: &Run) {
    use crate::rules::hop;
    let Some(h) = run.hop.as_ref().filter(|h| h.room == run.at) else {
        return;
    };
    let (cols, rows) = (run.room().cols as i32, run.room().rows as i32);
    let width = cols as f32 * TILE_UNITS;
    let t = h.t.saturating_sub(crate::rules::folly::TITLE);
    for r in 1..rows - 1 {
        let y = r * TILE;
        let Some(l) = hop::lane(rows, r) else {
            // A dry bank or the landing: flagstones.
            cv.rect(TILE, y, (cols - 2) * TILE, TILE, 'x');
            for x in (TILE..(cols - 1) * TILE).step_by(TILE as usize) {
                cv.line(x, y, x, y + TILE - 1, 's');
            }
            cv.line(TILE, y + TILE - 1, (cols - 1) * TILE - 1, y + TILE - 1, 's');
            continue;
        };
        cv.rect(TILE, y, (cols - 2) * TILE, TILE, 'q');
        let flow = (t as i32 * if l % 2 == 0 { 1 } else { -1 }) / 3;
        for x in (TILE..(cols - 1) * TILE).step_by(13) {
            let wx = TILE + (x - TILE + flow).rem_euclid((cols - 2) * TILE);
            cv.line(wx, y + 5 + (l as i32 % 2) * 5, wx + 4, y + 5 + (l as i32 % 2) * 5, 'f');
        }
        let (lefts, len) = hop::books(l, t, width);
        let spines = ['R', 'B', 'e', 'U', 'P'];
        // A book running off one end comes back in at the other.
        let span = lefts.len() as i32 * hop::SPACING as i32 * TILE;
        for (k, &bx) in lefts.iter().enumerate() {
            let x0 = (bx / TILE_UNITS * TILE as f32) as i32;
            let w = (len / TILE_UNITS * TILE as f32) as i32;
            for piece in [x0, x0 - span] {
                let (a, b) = (piece.max(TILE), (piece + w).min((cols - 1) * TILE));
                if a >= b {
                    continue;
                }
                cv.rect(a, y + 2, b - a, TILE - 4, spines[(k + l) % spines.len()]);
                cv.line(a, y + 3, b - 1, y + 3, '9');
                cv.line(a, y + TILE - 4, b - 1, y + TILE - 4, 'h');
                for sx in (a + 5..b - 2).step_by(9) {
                    cv.line(sx, y + 5, sx, y + TILE - 6, 'O');
                }
            }
        }
    }
    for &(x, y, left) in &h.splashes {
        let (px, py) = at(x, y);
        let rad = (hop_ripple(left) as f32).max(2.0);
        cv.ellipse(px as f32, py as f32, rad, rad / 2.0, 'U');
    }
}

fn hop_ripple(left: u32) -> u32 {
    3 + (crate::rules::HZ / 2 - left.min(crate::rules::HZ / 2)) / 2
}

/// Book Hop's board and title, over everything.
fn hop_layer(over: &mut Img, run: &Run) {
    let Some(h) = run.hop.as_ref().filter(|h| h.room == run.at) else {
        return;
    };
    let head = if h.done { "BOOK HOP  DONE".to_string() } else { format!("BOOK HOP  {}S", h.left()) };
    let rows: Vec<(u32, u32)> = h.crossings.iter().map(|(&id, &n)| (id, u32::from(n))).collect();
    folly_board(over, run, &head, &rows);
    if h.titled() {
        folly_title(over, "BOOK HOP", "HOP. DON'T SINK.");
    }
}

/// The clay pots along a room's walls, and the shards of broken ones.
fn clay_pots(cv: &mut Img, run: &Run) {
    for (_, (x, y)) in run.pots_of(run.at) {
        let (px, py) = at(x, y);
        // A squat pot: a dark mouth, a round belly lit from the left.
        cv.rect(px - 4, py + 5, 9, 1, 'q');
        cv.rect(px - 3, py - 2, 7, 7, 'P');
        cv.rect(px - 4, py, 9, 4, 'P');
        cv.rect(px - 3, py, 2, 3, 'o');
        cv.rect(px - 2, py - 4, 5, 2, 'B');
        cv.rect(px - 1, py - 4, 3, 1, 'b');
        cv.rect(px - 3, py + 3, 7, 1, 'B');
    }
    for &(x, y, left) in &run.pot_shards {
        let (px, py) = at(x, y);
        let spread = 1 + (crate::rules::pots::SHARDS - left) as i32 / 3;
        for (dx, dy, ink) in [(-2, 1, 'P'), (2, 0, 'o'), (0, 3, 'P'), (-1, -2, 'B'), (3, 3, 'o'), (-3, 3, 'B')] {
            cv.rect(px + dx * spread, py + dy + spread / 2, 2, 2, ink);
        }
    }
}

fn powder_layer(over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    let Some(p) = run.powder.as_ref().filter(|p| p.room == run.at) else {
        return;
    };
    let cols = run.room().cols as i32;
    let spot = |tx: i32, ty: i32| {
        let (x, y) = at(tx as f32 * TILE_UNITS, ty as f32 * TILE_UNITS);
        (x - ox, y - oy)
    };
    for &i in &p.crates {
        let (x, y) = spot(i as i32 % cols, i as i32 / cols);
        over.rect(x + 1, y + 1, TILE - 2, TILE - 2, 'r');
        over.rect(x + 2, y + 2, TILE - 4, TILE - 4, 'R');
        over.line(x + 2, y + 2, x + TILE - 3, y + TILE - 3, 'r');
        over.line(x + TILE - 3, y + 2, x + 2, y + TILE - 3, 'r');
        over.rect(x + 1, y + 1, TILE - 2, 1, 'o');
    }
    for k in &p.kegs {
        let (x, y) = spot(k.at.0, k.at.1);
        over.rect(x + 4, y + 5, 8, 9, 'K');
        over.rect(x + 4, y + 7, 8, 1, 'G');
        over.rect(x + 4, y + 11, 8, 1, 'G');
        // The fuse burns down, sparking.
        let spark = if run.tick / 3 % 2 == 0 { '6' } else { '7' };
        over.put(x + 9, y + 3, spark);
        over.put(x + 8, y + 4, 'h');
    }
    for &((tx, ty), left) in &p.fire {
        let (x, y) = spot(tx, ty);
        let ink = if left > 6 { '6' } else { '7' };
        over.rect(x + 2, y + 2, TILE - 4, TILE - 4, ink);
        over.rect(x + 5, y + 5, TILE - 10, TILE - 10, '9');
    }
    let head = if p.done { "POWDER  DONE".to_string() } else { format!("POWDER  {}S", p.left()) };
    let rows: Vec<(u32, u32)> = p.broken.iter().map(|(&id, &n)| (id, u32::from(n))).collect();
    folly_board(over, run, &head, &rows);
    if p.titled() {
        folly_title(over, "THE POWDER ROOM", "BOMB THE CRATES.");
    }
}

/// A folly's standings, top right: a headline, then each knight's colour,
/// name and score.
fn folly_board(over: &mut Img, run: &Run, head: &str, rows: &[(u32, u32)]) {
    let (w, row) = (118, 11);
    let x0 = NATIVE_W - w - 4;
    over.rect(x0 - 2, 2, w + 4, row * (rows.len() as i32 + 1) + 3, 'K');
    over.text(x0, 4, head, '6');
    for (k, &(id, n)) in rows.iter().enumerate() {
        let y = 4 + (k as i32 + 1) * row;
        let ink = ['5', '1', 'A', '4'][(id as usize).clamp(1, 4) - 1];
        over.rect(x0, y + 1, 5, 5, ink);
        let name: String = run
            .players
            .get(&id)
            .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
            .chars()
            .take(9)
            .collect();
        over.text(x0 + 8, y, &name, 'O');
        let count = format!("{n}");
        over.text(x0 + w - super::ink::text_width(&count), y, &count, '9');
    }
}

/// A folly's title card: its name, and how to play in a few words.
fn folly_title(over: &mut Img, a: &str, b: &str) {
    let tw = super::ink::text_width(b).max(super::ink::text_width(a));
    // High in the view: the middle is where an unused wish waits.
    let (bx, by) = ((NATIVE_W - tw) / 2 - 8, 44);
    over.rect(bx, by, tw + 16, 34, 'K');
    over.rect(bx, by, tw + 16, 1, '6');
    over.rect(bx, by + 33, tw + 16, 1, '6');
    over.text((NATIVE_W - super::ink::text_width(a)) / 2, by + 7, a, '6');
    over.text((NATIVE_W - super::ink::text_width(b)) / 2, by + 20, b, 'O');
}

/// Everyone plays at once: the standings, top right, live — each knight's
/// colour, name and how far along they are.
pub(super) fn heat_board(over: &mut Img, run: &Run) {
    let Some(heat) = run.pastimes.heat.as_ref().filter(|h| h.seats.len() > 1) else {
        return;
    };
    let need = i32::from(heat.game.need()).max(1);
    let (w, row) = (118, 11);
    let x0 = NATIVE_W - w - 4;
    let y0 = 4;
    over.rect(x0 - 2, y0 - 2, w + 4, row * heat.seats.len() as i32 + 3, 'K');
    for (i, &id) in heat.seats.iter().enumerate() {
        let y = y0 + i as i32 * row;
        let ink = ['5', '1', 'A', '4'][(id as usize).clamp(1, 4) - 1];
        over.rect(x0, y + 1, 5, 5, ink);
        let name: String = run
            .players
            .get(&id)
            .map_or_else(|| format!("SEAT {id}"), |h| h.name.to_uppercase())
            .chars()
            .take(9)
            .collect();
        over.text(x0 + 8, y, &name, 'O');
        let got = i32::from(heat.standing.get(&id).copied().unwrap_or(0));
        let bar = 40;
        over.rect(x0 + w - bar, y + 2, bar, 3, 'g');
        over.rect(x0 + w - bar, y + 2, bar * got / need, 3, ink);
    }
}

/// The way on, below: once a room is clear, an arrow in the doorway that
/// leads toward the floor's next aim (`Run::way_on`). It sits on the wall
/// itself, its words along the wall, so it never covers the room's boards.
fn way_on_marker(over: &mut Img, run: &Run, (ox, oy): (i32, i32), tick: u32) {
    let Some((dir, name)) = run.way_on() else {
        return;
    };
    let room = run.room();
    let (w, h) = (room.cols as f32 * TILE_UNITS, room.rows as f32 * TILE_UNITS);
    let wall = 0.5 * TILE_UNITS;
    let (ux, uy) = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)][dir];
    let (x, y) = match dir {
        0 => (w / 2.0, wall),
        1 => (w - wall, h / 2.0),
        2 => (w / 2.0, h - wall),
        _ => (wall, h / 2.0),
    };
    let nudge = [0.0, 1.0, 2.0, 1.0][(tick / 6 % 4) as usize];
    let (px, py) = at(x, y);
    let (sx, sy) = ((px - ox) as f32 + ux * nudge, (py - oy) as f32 + uy * nudge);
    let (sx, sy) = (sx.clamp(10.0, (NATIVE_W - 10) as f32), sy.clamp(10.0, (NATIVE_H - 10) as f32));
    for (ink, grow) in [('K', 1.5), ('6', 0.0)] {
        for dy in -9..=9 {
            for dx in -9..=9 {
                let along = dx as f32 * ux + dy as f32 * uy;
                let across = (dy as f32 * ux - dx as f32 * uy).abs();
                if (-4.0 - grow..=6.0 + grow).contains(&along)
                    && across <= (6.0 + grow - along) * 0.5 + grow * 0.5
                {
                    over.put(sx.round() as i32 + dx, sy.round() as i32 + dy, ink);
                }
            }
        }
    }
    // The words run along the wall beside the doorway.
    let label = name.to_uppercase();
    let tw = super::ink::text_width(&label);
    let (lx, ly) = match dir {
        0 | 2 => (sx as i32 + 2 * TILE, sy as i32 - 3),
        _ => (
            if dir == 1 { sx as i32 - tw - 12 } else { sx as i32 + 12 },
            sy as i32 - 2 * TILE,
        ),
    };
    let (lx, ly) = (lx.clamp(4, NATIVE_W - tw - 4), ly.clamp(4, NATIVE_H - 13));
    over.rect(lx - 2, ly - 2, tw + 4, 11, 'K');
    over.text(lx, ly, &label, '6');
}

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
        let len = dx.hypot(dy).max(0.01);
        let (ux, uy) = (dx / len, dy / len);
        for py in -4..=4 {
            for px in -4..=4 {
                // Along the way out (tip at +2) and across it.
                let along = px as f32 * ux + py as f32 * uy;
                let across = (py as f32 * ux - px as f32 * uy).abs();
                if (-3.0..=2.0).contains(&along) && across <= (2.0 - along) * 0.75 + 0.3 {
                    over.put(ex.round() as i32 + px, ey.round() as i32 + py, ink);
                }
            }
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
/// How near (in tiles) a knight stands to what a big board is about before
/// the board shows.
const READING_REACH: f32 = 3.5;

/// A standing knight within `reach` tiles of tile point `spot`.
pub(super) fn knight_near(run: &Run, spot: (f32, f32), reach: f32) -> bool {
    run.players.values().any(|h| {
        h.hp > 0
            && (h.x - spot.0 * TILE_UNITS).hypot(h.y - spot.1 * TILE_UNITS) < reach * TILE_UNITS
    })
}

/// The big boards (a game's rules, an errand, a plot's price) keep out of
/// the way: they show while a game is on, or while a knight stands by a
/// game's ring, a building plot or a road's stair. Walking about, the view
/// is the world's.
fn reading(run: &Run) -> bool {
    use crate::rules::{countryside, pastimes::Game, world};
    if run.pastimes.round.is_some() || coastal::board_visible(run) || woodland::board_visible(run)
        || wetland::board_visible(run)
        || aerie::board_visible(run)
        || windcountry::heist_board(run)
        || bowcountry::board_visible(run)
        || town::board_visible(run)
    {
        return true;
    }
    let kind = run.room().kind;
    Game::ALL
        .iter()
        .filter(|g| g.room() == kind)
        .map(|g| g.start())
        .chain(countryside::PLOTS.iter().filter(|p| p.room == kind).map(|p| p.at))
        .chain(
            world::ENTRANCES
                .iter()
                .filter(|e| e.room == kind)
                .map(world::entrance_centre),
        )
        .any(|spot| knight_near(run, spot, READING_REACH))
}

fn layers(run: &Run, focus: Option<u32>, legible: bool) -> (Img, Img) {
    layers_with(run, focus, legible, false)
}

fn layers_with(run: &Run, focus: Option<u32>, legible: bool, modern: bool) -> (Img, Img) {
    let (scenery, mut over) = scene_layers_with(run, focus, legible, modern);
    votes::panel(&mut over, run, focus);
    (scenery, over)
}

fn scene_layers_with(run: &Run, focus: Option<u32>, legible: bool, modern: bool) -> (Img, Img) {
    if run.room().kind == RoomKind::Kingdom {
        return kingdom::layers_with(run, focus, legible, modern);
    }
    debug_assert_eq!(
        (NATIVE_W, NATIVE_H),
        at(WIDTH, HEIGHT),
        "a tile is two arena units"
    );
    // A dimensional pastime takes the whole canvas: its own screen, not a
    // corner of the room.
    if stage::takeover(run) {
        return stage::layers(run, focus, legible);
    }
    let (pw, ph) = room_px(run.room());
    let mut over = Img::new(pw, ph);
    let mut scene = DrawList::default();
    scene.add(Layer::Ground, |cv| hazards(cv, run));
    let fortune_hall = run.room().kind == RoomKind::Fortune;
    let in_yard = run.room().kind == RoomKind::Yard;
    let in_hall = run.room().kind == RoomKind::Trophies;
    let in_tavern = run.room().kind == RoomKind::Tavern;
    scene.add(Layer::Ground, |cv| {
        if run.at_home_now() {
            if fortune_hall {
                fortune::figures(cv, run);
            } else if in_yard {
                yard::figures(cv, run);
                lessons::figures(cv, run);
            } else if in_hall {
                trophies::figures(cv, run);
            } else if in_tavern {
                tavern::figures(cv, run);
            } else if run.room().kind == RoomKind::Home {
                home::figures(cv, run);
                rescues::residents(cv, run);
                hireling::at_the_stair(cv, run);
            }
            match run.room().kind {
                RoomKind::Stables => joust::stables(cv, run),
                RoomKind::Lists => joust::lists(cv, run, legible),
                RoomKind::Forest => forest::figures(cv, run),
                RoomKind::Eyrie => race3d::eyrie_figures(cv, run),
                RoomKind::LandOffice => holdings::office(cv, run),
                RoomKind::RealmHall => holdings::castle(cv, run),
                _ => {}
            }
            countryside::figures(cv, run);
            pastimes::figures(cv, run);
            world::entrance_ring(cv, run);
        }
    });
    scene.add(Layer::Ground, |cv| {
        if matches!(
            run.room().kind,
            RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Gate | RoomKind::Forge
        ) {
            barony::plates(cv, run);
            barony::folk(cv, run);
        }
    });
    scene.add(Layer::Ground, |cv| {
        if !run.at_home_now() {
            // A floor the realm holds: its banners, its steward's hall.
            holdings::start_banners(cv, run);
            if run.room().kind == RoomKind::Holding {
                holdings::hall(cv, run);
            }
            nooks::figures(cv, run);
        }
    });
    scene.add(Layer::Ground, |cv| college::figures(cv, run));
    scene.add(Layer::Ground, |cv| ranch::figures_with(cv, run, modern));
    scene.add(Layer::Ground, |cv| works::under_with(cv, run, modern));
    scene.add(Layer::Ground, |cv| merchant::figures(cv, run));
    scene.add(Layer::Ground, |cv| ults::under(cv, run));
    scene.add(Layer::Ground, |cv| foes_hazards(cv, run));
    scene.add(Layer::Ground, |cv| critters::world(cv, run));
    scene.add(Layer::Ground, |cv| tide::ravages(cv, run));
    scene.add(Layer::Ground, |cv| void::holes(cv, run));
    scene.add(Layer::Ground, |cv| rescues::cage(cv, run));
    scene.add(Layer::Ground, |cv| hand::ground_layer(cv, run));
    scene.add(Layer::Ground, |cv| gate::draw(cv, run));
    scene.add(Layer::Ground, |cv| feel::under(cv, run));
    scene.add(Layer::Ground, |cv| clay_pots(cv, run));
    scene.add(Layer::Ground, |cv| hop_floor(cv, run));
    scene.add(Layer::Ground, |cv| signatures::under_with(cv, run, modern));
    scene.add(Layer::Figures, |cv| figures_with(cv, run, modern));
    scene.add(Layer::Over, |cv| aerie::poop(cv, run));
    scene.add(Layer::Over, |cv| {
        if legible {
            pitstop::board(cv, run);
            frogs::plates(cv, run);
        }
    });
    scene.add(Layer::Over, |cv| wanted::nameplates(cv, run));
    scene.add(Layer::Over, |cv| marks::take_prompts(cv, run, legible));
    scene.add(Layer::Over, |cv| marks::thief_cries(cv, run, legible));
    scene.add(Layer::Over, |cv| tallow(cv, run));
    scene.add(Layer::Over, |cv| hooks(cv, run));
    scene.add(Layer::Over, |cv| fan_boxes(cv, run));
    scene.add(Layer::Over, |cv| sparks(cv, run));
    scene.add(Layer::Over, |cv| orbits(cv, run));
    scene.add(Layer::Over, |cv| summoned(cv, run));
    scene.add(Layer::Over, |cv| flourishes(cv, run));
    scene.add(Layer::Over, |cv| feel::over(cv, run));
    scene.add(Layer::Over, |cv| works::over(cv, run));
    scene.add(Layer::Over, |cv| signatures::over(cv, run));
    scene.add(Layer::Over, |cv| wanted::over(cv, run));
    scene.add(Layer::Over, |cv| ults::over(cv, run));
    scene.add(Layer::Boards, |cv| {
        if run.at_home_now() && legible {
            if fortune_hall {
                fortune::boards(cv, run);
            } else if in_yard {
                yard::boards(cv, run);
                lessons::boards(cv, run);
            } else if in_hall {
                trophies::boards(cv, run);
                tavern::wing_ledger(cv, run);
            } else if in_tavern {
                tavern::boards(cv, run);
            } else if run.room().kind == RoomKind::Home {
                home::boards(cv, run);
            }
            if run.room().kind.in_world() || run.room().kind == RoomKind::Home {
                world::boards(cv, run);
            }
            match run.room().kind {
                RoomKind::Stables => joust::stables_boards(cv, run),
                RoomKind::Lists => joust::lists_boards(cv, run, focus),
                RoomKind::Forest => forest::boards(cv, run),
                RoomKind::LandOffice => holdings::office_boards(cv, run, focus),
                RoomKind::RealmHall => holdings::castle_boards(cv, run, focus),
                _ => {}
            }
            if reading(run) {
                pastimes::boards(cv, run);
                countryside::boards(cv, run);
            }
        }
    });
    scene.add(Layer::Boards, |cv| {
        if legible {
            merchant::boards(cv, run);
            if run.room().kind == RoomKind::Holding {
                holdings::hall_boards(cv, run, focus);
            }
            if !run.at_home_now() {
                nooks::boards(cv, run);
            }
        }
    });
    scene.add(Layer::Boards, |cv| {
        if legible
            && matches!(
                run.room().kind,
                RoomKind::MineHead | RoomKind::KingsHall | RoomKind::Forge
            )
        {
            barony::boards(cv, run);
        }
    });
    scene.paint(&mut over);
    let banner = legible.then(|| run.banner_now()).flatten();
    let origin = view_origin(run, focus);
    let mut over = crop(&over, origin);
    let mut scenery = if modern { Img::new(NATIVE_W, NATIVE_H) } else { crop(&scenery_cached(run), origin) };
    if run.circle() > 0 && !run.at_home_now() {
        // Below a hell gate the rooms are lit from underneath.
        gate::tint(&mut scenery, run.circle());
    }
    if run.mode == crate::rules::fortune::Mode::LightsOut && !run.at_home_now() {
        lights_out(&mut scenery, &mut over, run, origin);
    }
    // The screen's own marks, after the crop.
    let mut screen = DrawList::default();
    screen.add(Layer::Screen, |cv| {
        if legible {
            // The tray goes under the edge marks, which must stay in sight.
            tray::corner(cv, run, focus);
        }
    });
    screen.add(Layer::Screen, |cv| beyond(cv, run, origin));
    screen.add(Layer::Screen, |cv| way_on_marker(cv, run, origin, run.tick as u32));
    screen.add(Layer::Screen, |cv| {
        if let Some(((x, y), name)) = run.home_way() {
            // Bobbing over the Winding Stair, pointing down into it.
            let bob = [0.0, 1.0, 2.0, 1.0][(run.tick / 6 % 4) as usize];
            let (px, py) = at(x, y);
            let (sx, sy) = ((px - origin.0) as f32, (py - origin.1) as f32 - 44.0 - bob);
            pointer(cv, (sx, sy), (0.0, 1.0), name, false);
        }
    });
    screen.add(Layer::Screen, |cv| heat_board(cv, run));
    screen.add(Layer::Screen, |cv| folly_layer(cv, run, origin));
    screen.add(Layer::Screen, |cv| bats_layer(cv, run));
    screen.add(Layer::Screen, |cv| gantry_layer(cv, run, origin));
    screen.add(Layer::Screen, |cv| powder_layer(cv, run, origin));
    screen.add(Layer::Screen, |cv| hop_layer(cv, run));
    screen.add(Layer::Screen, |cv| field_banner(cv, run, origin, focus));
    screen.add(Layer::Screen, |cv| verdict(cv, run));
    screen.add(Layer::Screen, |cv| gate::card(cv, run));
    screen.add(Layer::Screen, |cv| wanted::poster(cv, run, legible));
    screen.add(Layer::Screen, |cv| {
        if let Some(feat) = banner {
            achievement_banner(cv, run, feat);
        } else if legible {
            dare_board(cv, run);
        }
    });
    screen.add(Layer::Screen, |cv| {
        if !modern || !focus.or_else(|| run.players.keys().next().copied()).and_then(|id|run.players.get(&id)).is_some_and(|h|h.pit.result.as_ref().is_some_and(|r|r.left>=10)) {
            pitstop::panel(cv, run, focus, legible);
        }
    });
    screen.add(Layer::Screen, |cv| college::prompt(cv, run, focus, legible));
    screen.add(Layer::Screen, |cv| if legible { college::dialogue(cv, run); });
    screen.paint(&mut over);
    (scenery, over)
}

/// One camera origin shared by both art pipelines, including hit shake.
pub fn view_origin(run: &Run, focus: Option<u32>) -> (i32, i32) {
    let origin = camera(run, focus);
    if run.shake == 0 { return origin; }
    let k = run.shake.min(12) as i32 / 2;
    let h = super::ink::hash(run.tick as i32, run.shake as i32, 91);
    (origin.0 + (h % (2 * k as u32 + 1)) as i32 - k,
     origin.1 + ((h >> 8) % (2 * k as u32 + 1)) as i32 - k)
}

/// A warboar's tells: the line it marks before a charge, and the stars
/// over its head while it stands dazed.
fn foes_marks(cv: &mut Img, run: &Run, enemy: &crate::rules::Enemy, tick: u32) {
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
    use crate::rules::foes::{FLING_FALL, KEG_ARMED, LOB_FALL, WELL_REACH};
    use crate::rules::hunters::{HOOK_RANGE, WEB_REACH};
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
        use crate::rules::pit::{SLAM_REACH, SLAM_WINDUP};
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
    for well in &run.wells {
        let (wx, wy) = at(well.x, well.y);
        // The lean's reach: a fading ring, and grit turning in the bend.
        let reach = (WELL_REACH * UNIT * 0.9) as i32;
        for i in 0..64 {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            let r = reach as f32 * (0.75 + 0.25 * (a * 3.0 + run.tick as f32 * 0.05).sin());
            cv.put(
                wx + (a.cos() * r) as i32,
                wy + (a.sin() * r * 0.7) as i32,
                if i % 8 == 0 { 'u' } else { 'x' },
            );
        }
        for arm in 0..3 {
            let a = run.tick as f32 * 0.11 + arm as f32 * (std::f32::consts::TAU / 3.0);
            for k in 3..8 {
                let r = reach as f32 * k as f32 / 10.0;
                cv.put(
                    wx + (a.cos() * r) as i32,
                    wy + (a.sin() * r * 0.7) as i32,
                    'U',
                );
            }
        }
    }
    for fling in &run.flings {
        let (tx, ty) = at(fling.x, fling.y);
        let r = 17.0;
        let n = 48;
        for i in (0..n).step_by(3) {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            cv.put(tx + (a.cos() * r) as i32, ty + (a.sin() * r * 0.6) as i32, '8');
        }
        let (fx, fy) = at(fling.from.0, fling.from.1);
        let k = 1.0 - fling.fall as f32 / FLING_FALL as f32;
        let arc = (k * std::f32::consts::PI).sin() * 20.0;
        let (bx, by) = (
            fx + ((tx - fx) as f32 * k) as i32,
            fy + ((ty - fy) as f32 * k) as i32 - arc as i32,
        );
        let im = sprites::flung_impling(tick);
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
    // A scratch behind the ears: two hearts float up and fade.
    let since = run.tick.saturating_sub(cat.petted);
    if cat.petted > 0 && since < crate::rules::cat::PURR {
        let rise = (since / 3) as i32;
        let heart = ["7.7", "777", ".7."];
        for (k, dx) in [(0, -4), (1, 3)] {
            let hy = y - im.h - 2 - rise - k * 4;
            for (ry, row) in heart.iter().enumerate() {
                for (rx, c) in row.chars().enumerate() {
                    if c != '.' {
                        cv.put(x + dx + rx as i32, hy + ry as i32, c);
                    }
                }
            }
        }
    }
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
    use crate::rules::audience::BOX_FALL;
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
fn achievement_banner(cv: &mut Img, run: &Run, feat: &crate::rules::feats::Feat) {
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
    use crate::rules::dares::CALLED;
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
pub fn frame(run: &Run, w: i32, h: i32) -> Img {
    frame_for(run, w, h, None)
}

/// The view a given knight's camera shows (`None`: between the knights).
/// Where an arena point lands in the `w`x`h` frame `frame_for` draws of
/// `run` for `focus` (the screen shake left out): for a client's probes.
pub fn frame_px(run: &Run, w: i32, h: i32, focus: Option<u32>, ux: f32, uy: f32) -> (i32, i32) {
    let (cx, cy) = camera(run, focus);
    let (nx, ny) = at(ux, uy);
    Field::fit(w, h).map(nx - cx, ny - cy)
}

/// Who is here, on a frame already drawn (`rules::presence`): "AWAY" over
/// each knight whose player is not, and a banner while the room holds for
/// its party. Drawn last, over either art, on every seat's own frame.
pub fn presence_marks(img: &mut Img, run: &Run, focus: Option<u32>) {
    let (w, h) = (img.w, img.h);
    let tw = super::ink::text_width("AWAY") + 4;
    // Knights side by side stack their tags rather than overprint them.
    let mut tags: Vec<(i32, i32)> = Vec::new();
    for hero in run.players.values().filter(|h| h.away && h.hp > 0) {
        let (x, y) = frame_px(run, w, h, focus, hero.x, hero.y);
        let (bx, mut by) = (x - tw / 2, y - 30);
        while tags.iter().any(|&(tx, ty)| (tx - bx).abs() < tw && (ty - by).abs() < 12) {
            by -= 12;
        }
        if bx < 0 || by < 0 || bx + tw > w || by + 11 > h {
            continue;
        }
        tags.push((bx, by));
        img.rect(bx, by, tw, 11, 's');
        img.text(bx + 2, by + 2, "AWAY", 'T');
    }
    // A fallen or finished run says so itself.
    if !run.active() {
        return;
    }
    let banner = match run.held {
        crate::rules::Hold::Open => return,
        crate::rules::Hold::Arriving => "THE PARTY GATHERS",
        crate::rules::Hold::Empty => "THE ROOM WAITS FOR ITS KNIGHTS",
    };
    let bw = super::ink::text_width(banner) + 20;
    if bw > w || h < 40 {
        return;
    }
    let x = (w - bw) / 2;
    img.rect(x, 12, bw, 21, 's');
    img.frame(x, 12, bw, 21, 'o');
    img.text(x + 10, 19, banner, 'T');
}

pub fn frame_for(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    if art_v2::enabled() && art_v2::supported(run) && run.floor_plate(focus).is_none() {
        return art_v2::frame(run, w, h, focus);
    }
    frame_v1(run, w, h, focus)
}

pub fn frame_v1(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Img {
    if let Some(plate) = run.floor_plate(focus) {
        return plates::frame(plate, w, h);
    }
    let mut out = Img::black(w.max(0), h.max(0));
    if w < 12 || h < 8 {
        return out;
    }
    let legible = w >= NATIVE_W && h >= NATIVE_H;
    let (scenery, over) = layers(run, focus, legible);
    let field = Field::fit(w, h);
    if run.room().kind == RoomKind::Kingdom && field.den != 1 {
        let ranks = super::kingdom::atlas_ranks(&run.kingdom.wishes);
        field.blit_ranked(
            &scenery,
            &over,
            &mut out,
            Some((
                &ranks,
                super::MAP_W as usize * TILE as usize,
                camera(run, focus),
            )),
        );
    } else {
        field.blit(&scenery, &over, &mut out);
    }
    out
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__arena_tests.rs"]
mod tests;

/// Snapshot only on worker admission; the expensive compose is lazy.
pub struct LazyFrame {
    run: Run,
    w: u32,
    h: u32,
    focus: Option<u32>,
    rgba: std::sync::OnceLock<Vec<u8>>,
}
impl LazyFrame {
    pub fn new(run: Run, w: u32, h: u32, focus: Option<u32>) -> Self {
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

#[cfg(any(test, feature = "testing"))]
pub fn privy_panel_for_test(cv: &mut Img, run: &Run, focus: Option<u32>) {
    pitstop::panel(cv, run, focus, true);
}
