//! The Works, drawn: every building of the hillside in flat code art, in
//! every state it can be in, plus the things that run on its lines and the
//! hands that work in it (docs/KINGDOM-WORKS.md, ECONOMY.md). 16 px a tile,
//! the realm's palette, no pasted pictures: a pure function of
//! `(plan, state, frame)`.
//!
//! - [`flat`] is the one lookup: a plan's id (the sim's, `lime-kiln`), its
//!   [`State`] and the animation step; a picture as wide as the footprint
//!   and as tall as its depth plus what rises over it, its bottom edge on
//!   the footprint's. Pictures are made once and shared.
//! - States: **under construction** in four steps (pegged out, footings,
//!   walls, roofing) drawn from the finished building itself so what is
//!   being raised is plain; **idle** (a stopped building: fires out, tools
//!   down); **working** (every moving part moving, [`FRAMES`] steps a
//!   loop); **starved** (cold and still, a board over the door with what is
//!   missing); **backed up** (stock piled outside, an amber board).
//! - [`lines`]: the laid line per kind and what runs on it, a load on
//!   each. [`hands`]: the people, by trade, on the knights' rig.
//! - [`Load`]: what a thing carries, one look per family of goods.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::ink::{Img, hash};

mod commons;
mod craft;
mod defence;
mod fire;
mod gaps;
mod hall;
pub mod hands;
mod loads;
pub mod lines;
mod paint;
pub mod rollers;
mod power;
mod stone;
mod timber;
mod yard;
mod yards;

pub use defence::{blaze, cracks};
pub use hands::{Act, Pose, Trade};
pub use loads::Load;

#[cfg(test)]
#[path = "../../../../tests/render/works_kit__tests.rs"]
mod tests;

/// Steps in a working loop. A caller picks the rate: eight a second reads
/// as work; `frame` is any count and wraps.
pub const FRAMES: u32 = 8;

/// Steps a building is raised in: 0 pegged out, 1 footings, 2 walls up,
/// 3 roofing.
pub const STEPS: u8 = 4;

/// What a building is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// Being raised, step `0..STEPS`.
    Building(u8),
    /// Finished and stopped.
    Idle,
    /// Running: every moving part moving.
    Working,
    /// Cold: this good is not coming in.
    Starved(Load),
    /// Stock piled outside: this good cannot go out.
    BackedUp(Load),
}

impl State {
    /// How many pictures a loop of it has: [`FRAMES`] working, else one.
    pub fn frames(self) -> u32 {
        if self == State::Working { FRAMES } else { 1 }
    }
}

/// What a building is raised of: what its construction piles up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mat {
    Stone,
    Timber,
    Brick,
    /// Dug, not built: pits, ponds, ditches.
    Earth,
}

/// What one drawing is given.
pub(crate) struct Ctx {
    pub im: Img,
    pub w: i32,
    pub h: i32,
    /// The footprint's depth in pixels: the bottom rows of the picture.
    pub d: i32,
    /// The animation step.
    pub t: u32,
    /// Running.
    pub work: bool,
    pub seed: u32,
}

impl Ctx {
    /// The last row of the picture: the footprint's front edge.
    pub fn foot(&self) -> i32 {
        self.h - 1
    }

    /// The first row of the footprint.
    pub fn yard(&self) -> i32 {
        self.h - self.d
    }
}

pub(crate) struct Plan {
    pub id: &'static str,
    /// The footprint in tiles, width by depth.
    pub w: i32,
    pub d: i32,
    /// Pixels the picture rises over its footprint.
    pub rise: i32,
    pub mat: Mat,
    pub draw: fn(&mut Ctx),
}

const fn plan(id: &'static str, w: i32, d: i32, rise: i32, mat: Mat, draw: fn(&mut Ctx)) -> Plan {
    Plan { id, w, d, rise, mat, draw }
}

use Mat::{Brick, Earth, Stone, Timber};

/// Every plan of the hillside's catalogue (art/plans.txt), in its order, then
/// the carter's yard, the castle's stockyard, the camp and the shaft heads.
pub(crate) static PLANS: &[Plan] = &[
    plan("field-quarry", 3, 2, 10, Stone, stone::field_quarry),
    plan("rubble-pit", 2, 2, 4, Earth, stone::rubble_pit),
    plan("claim-quarry", 3, 3, 24, Stone, stone::claim_quarry),
    plan("banker", 3, 2, 16, Stone, stone::banker),
    plan("tracing-floor", 4, 3, 8, Stone, stone::tracing_floor),
    plan("slater", 2, 2, 12, Stone, stone::slater),
    plan("lime-kiln", 2, 2, 22, Stone, fire::lime_kiln),
    plan("slaking-pit", 2, 2, 6, Earth, yard::slaking_pit),
    plan("mortar-mill", 2, 2, 14, Timber, power::mortar_mill),
    plan("sand-pit", 2, 2, 6, Earth, yard::sand_pit),
    plan("well", 1, 1, 18, Stone, commons::well),
    plan("intake", 2, 2, 12, Timber, power::intake),
    plan("clamp", 2, 2, 12, Earth, fire::clamp),
    plan("woodsman-camp", 3, 2, 18, Timber, timber::woodsman_camp),
    plan("sawpit", 3, 2, 20, Timber, timber::sawpit),
    plan("carpentry", 3, 2, 22, Timber, timber::carpentry),
    plan("wheelwright", 3, 2, 22, Timber, timber::wheelwright),
    plan("retting-pond", 3, 2, 6, Earth, yard::retting_pond),
    plan("ropewalk", 6, 2, 16, Timber, timber::ropewalk),
    plan("farmstead", 4, 3, 26, Timber, craft::farmstead),
    plan("mill", 2, 2, 26, Stone, craft::mill),
    plan("bakery", 2, 2, 26, Brick, craft::bakery),
    plan("brewery", 3, 2, 30, Timber, craft::brewery),
    plan("pasture", 4, 3, 8, Earth, yard::pasture),
    plan("tanyard", 3, 2, 14, Earth, yard::tanyard),
    plan("weaving-shed", 3, 2, 22, Timber, craft::weaving_shed),
    plan("ashery", 2, 2, 12, Earth, fire::ashery),
    plan("glassworks", 3, 2, 26, Stone, fire::glassworks),
    plan("bloomery", 2, 3, 30, Stone, fire::bloomery),
    plan("forge-shop", 3, 2, 30, Stone, fire::forge_shop),
    plan("smithy", 2, 2, 18, Timber, fire::smithy),
    plan("lead-works", 2, 2, 18, Stone, fire::lead_works),
    plan("lodge", 4, 2, 26, Timber, hall::lodge),
    plan("ditch-gang", 3, 2, 4, Earth, yard::ditch_gang),
    plan("depot", 3, 2, 14, Timber, hall::depot),
    plan("quay", 3, 2, 16, Stone, hall::quay),
    plan("hut", 2, 1, 16, Timber, hall::hut),
    plan("treadwheel-crane", 2, 2, 40, Timber, power::treadwheel_crane),
    plan("treadwheel", 2, 1, 22, Timber, power::treadwheel),
    plan("horse-gin", 3, 3, 20, Timber, power::horse_gin),
    plan("windmill", 2, 2, 40, Timber, power::windmill),
    plan("cottage", 2, 1, 22, Timber, hall::cottage),
    plan("mason", 3, 2, 16, Stone, stone::mason_yard),
    plan("warehouse", 4, 2, 28, Timber, hall::warehouse),
    plan("caravanserai", 5, 3, 28, Stone, hall::caravanserai),
    plan("wall", 1, 1, 16, Stone, hall::wall),
    plan("clay-pit", 2, 2, 6, Earth, yard::clay_pit),
    plan("brick-kiln", 3, 2, 24, Brick, fire::brick_kiln),
    plan("brick-cottage", 2, 1, 24, Brick, hall::brick_cottage),
    plan("brick-wall", 1, 1, 16, Brick, hall::brick_wall),
    plan("brick-gate", 2, 1, 24, Brick, hall::brick_gate),
    plan("waterwheel", 3, 2, 24, Timber, power::waterwheel),
    plan("drive-shaft", 1, 1, 8, Timber, power::drive_shaft),
    plan("leyline", 1, 1, 4, Stone, power::leyline),
    plan("mana-engine", 2, 2, 22, Stone, power::mana_engine),
    plan("conduit", 1, 1, 14, Stone, power::conduit),
    plan("enchanter", 2, 2, 44, Stone, power::enchanter),
    plan("forge", 4, 2, 32, Stone, fire::royal_forge),
    // What the first kit left undrawn: the lift's first and last steps, the vines, the arcade and the swape.
    plan("hod-gang", 1, 1, 24, Timber, gaps::hod_gang),
    plan("great-crane", 1, 1, 56, Timber, gaps::great_crane),
    plan("vineyard", 1, 1, 10, Earth, gaps::vineyard),
    plan("wine-press", 1, 1, 22, Timber, gaps::wine_press),
    plan("aqueduct", 7, 1, 28, Stone, gaps::aqueduct),
    plan("swape", 1, 1, 12, Timber, gaps::swape),
    // What stands against the Deep (`rules::deep_raid`).
    plan("palisade", 1, 1, 14, Timber, defence::palisade),
    plan("wood-gate", 2, 1, 22, Timber, defence::wood_gate),
    plan("watchtower", 2, 2, 44, Timber, defence::watchtower),
    plan("brazier", 1, 1, 14, Stone, defence::brazier),
    // The sim's yards and the founding camp (one tile: the sim keeps it so).
    plan("carters-yard", 3, 2, 18, Timber, yards::carters_yard),
    plan("castle-yard", 3, 2, 28, Stone, yards::castle_yard),
    plan("camp", 1, 1, 28, Timber, yards::camp),
    // The heads of the Great Shaft's lift, one a tier, a tile each.
    plan("shaft-hand", 1, 1, 22, Timber, yards::shaft_hand),
    plan("shaft-windlass", 1, 1, 22, Timber, yards::shaft_windlass),
    plan("shaft-gin", 1, 1, 24, Timber, yards::shaft_gin),
    plan("shaft-water", 1, 1, 26, Timber, yards::shaft_water),
    plan("shaft-ley", 1, 1, 30, Stone, yards::shaft_ley),
];

pub(crate) fn find(id: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|p| p.id == id)
}

/// Every plan id this kit draws.
pub fn plans() -> impl Iterator<Item = &'static str> {
    PLANS.iter().map(|p| p.id)
}

/// The footprint of plan `id` in tiles, width and depth.
pub fn size(id: &str) -> Option<(i32, i32)> {
    find(id).map(|p| (p.w, p.d))
}

/// The picture's size in pixels: the footprint's width, its depth and what
/// rises over it. Its bottom edge stands on the footprint's.
pub fn sprite_size(id: &str) -> Option<(i32, i32)> {
    find(id).map(|p| (p.w * 16, p.d * 16 + p.rise))
}

/// Plan `id` in `state` at animation step `frame` (any count, it wraps),
/// or nothing for a plan this kit does not draw. Made once and shared.
pub fn flat(id: &str, state: State, frame: u32) -> Option<Arc<Img>> {
    let index = PLANS.iter().position(|p| p.id == id)?;
    let frame = if state.frames() > 1 { frame % state.frames() } else { 0 };
    static MADE: OnceLock<Mutex<HashMap<(usize, State, u32), Arc<Img>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    let key = (index, state, frame);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return Some(hit);
    }
    let made = Arc::new(make(&PLANS[index], state, frame));
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    Some(made)
}

fn seed_of(id: &str) -> u32 {
    id.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)))
}

/// The finished building at step `t`, running or not.
fn body(p: &Plan, work: bool, t: u32) -> Img {
    let (w, h) = (p.w * 16, p.d * 16 + p.rise);
    let mut c = Ctx { im: Img::new(w, h), w, h, d: p.d * 16, t, work, seed: seed_of(p.id) };
    (p.draw)(&mut c);
    c.im
}

fn make(p: &Plan, state: State, t: u32) -> Img {
    match state {
        State::Idle => body(p, false, 0),
        State::Working => body(p, true, t),
        State::Building(step) => raise(p, step.min(STEPS - 1)),
        State::Starved(load) => {
            let mut im = body(p, false, 0);
            starved_sign(&mut im, load);
            im
        }
        State::BackedUp(load) => {
            let mut im = body(p, false, 0);
            backed_up(&mut im, p, load);
            im
        }
    }
}

/// The board over a starved building: what is not coming in, framed red.
fn starved_sign(im: &mut Img, load: Load) {
    let (x, y) = sign_at(im);
    sign(im, x, y, load, '7');
}

/// Where a plan's board stands: before its door, or, on a plan too narrow
/// for that (a well, a wall), over its top corner.
fn sign_at(im: &Img) -> (i32, i32) {
    if im.w <= 16 { (0, 0) } else { (1, im.h - 24) }
}

/// A board on a post: `load`'s picture inside a frame of `frame`.
fn sign(im: &mut Img, x: i32, y: i32, load: Load, frame: char) {
    for k in 0..8 {
        im.put(x + 5, y + 12 + k, 'o');
        im.put(x + 6, y + 12 + k, 'B');
    }
    im.rect(x, y, 12, 12, frame);
    im.rect(x + 1, y + 1, 10, 10, 'n');
    let icon = loads::icon(load);
    im.stamp(&icon, x + 1, y + 1);
}

/// Stock piled in the yard, the way out blocked: heaps of `load` in front
/// and to the right, and an amber board.
fn backed_up(im: &mut Img, p: &Plan, load: Load) {
    let (w, h) = (im.w, im.h);
    let seed = seed_of(p.id);
    loads::heap(im, w - 11, h - 1, 18, 12, load, seed);
    if w >= 40 {
        loads::heap(im, w - 28, h - 1, 14, 9, load, seed ^ 5);
    }
    if w >= 64 {
        loads::heap(im, w - 42, h - 1, 14, 8, load, seed ^ 9);
    }
    let (x, y) = sign_at(im);
    sign(im, x, y, load, '4');
}

/// A building being raised: step 0 pegs the ground out, the rest show the
/// finished building from its foot up to how far the work has got, with
/// the scaffold against it and its materials in the yard.
fn raise(p: &Plan, step: u8) -> Img {
    let fin = body(p, false, 0);
    let (w, h) = (fin.w, fin.h);
    let seed = seed_of(p.id);
    let mut im = Img::new(w, h);
    let footing = p.d * 16;
    match p.mat {
        Mat::Earth => {
            // Dug from one end: the finished work to the left of a ragged
            // line, pegs and cord to the right.
            let frac = [0.0, 0.3, 0.62, 0.9][step as usize];
            let edge = (w as f32 * frac) as i32;
            for y in 0..h {
                for x in 0..w {
                    let ragged = edge + (hash(y / 2, step as i32, seed) % 5) as i32 - 2;
                    if x < ragged {
                        if let Some(c) = fin.get(x, y) {
                            im.set(x, y, c);
                        }
                    }
                }
            }
            let pegs = |im: &mut Img, x0: i32| {
                let (y0, y1) = (h - footing + 3, h - 4);
                for (x, y) in [(x0, y0), (w - 4, y0), (x0, y1), (w - 4, y1)] {
                    paint::post(im, x, y - 5, y);
                }
                im.line(x0 + 1, y0 - 4, w - 3, y0 - 4, 't');
                im.line(x0 + 1, y1 - 4, w - 3, y1 - 4, 't');
            };
            if step == 0 {
                paint::earth(&mut im, w as f32 / 2.0, h as f32 - footing as f32 / 2.0, w as f32 * 0.45, footing as f32 * 0.4, seed);
            }
            pegs(&mut im, (edge + 2).min(w - 8));
            paint::spoil(&mut im, w - 8, h - 3, 7.0, 5.0, seed);
            hands::stamp(&mut im, (edge + 8).min(w - 12), h - 3, Trade::Labourer, Act::Idle, 0, false);
        }
        _ => {
            if step == 0 {
                // Pegged out: cord between four stakes round the footprint,
                // the ground scuffed, a heap of what it will be made of.
                paint::earth(&mut im, w as f32 / 2.0, h as f32 - footing as f32 / 2.0 - 2.0, w as f32 * 0.46, footing as f32 * 0.42, seed);
                let (y0, y1) = (h - footing + 4, h - 5);
                let (x0, x1) = (4, w - 5);
                for (x, y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                    paint::post(&mut im, x, y - 5, y);
                }
                for (a, b) in [((x0, y0), (x1, y0)), ((x0, y1), (x1, y1)), ((x0, y0), (x0, y1)), ((x1, y0), (x1, y1))] {
                    paint::rope(&mut im, a.0 + 1, a.1 - 4, b.0 + 1, b.1 - 4, 1.0, 't');
                }
                let load = match p.mat {
                    Mat::Timber => Load::Timber,
                    Mat::Brick => Load::Brick,
                    _ => Load::Stone,
                };
                paint::pile(&mut im, w - 14, h - 2, 16, 12, load, seed);
                hands::stamp(&mut im, 14.min(w / 3), h - 2, Trade::Labourer, Act::Work, 2, false);
            } else {
                let frac = [0.0, 0.2, 0.52, 0.86][step as usize];
                let cut = h - ((h - 2) as f32 * frac).round() as i32;
                for x in 0..w {
                    let top = cut + (hash(x / 3, step as i32, seed) % 3) as i32 - 1;
                    for y in top.max(0)..h {
                        if let Some(c) = fin.get(x, y) {
                            im.set(x, y, c);
                        }
                    }
                }
                // Roofing: bare rafters where the roof is not yet on.
                if step == 3 {
                    let r = (h as f32 * 0.14) as i32;
                    for k in 0..3 {
                        let x = 3 + k * (w - 8) / 2;
                        im.line(x, cut, w / 2, cut - r, 'o');
                        im.line(x + 1, cut, w / 2 + 1, cut - r, 'B');
                    }
                    im.line(w / 2, cut - r, w / 2, cut + 2, 'o');
                }
                // The scaffold against the face and its planks.
                let top = (cut - 12).max(1);
                let rows = [cut + 1, (cut + h) / 2 - 2];
                paint::scaffold(&mut im, &[2, w / 2, w - 4], top, h - 1, &rows);
                paint::ladder(&mut im, w - 10, top + 3, h - 2);
                let load = match p.mat {
                    Mat::Timber => Load::Planks,
                    Mat::Brick => Load::Brick,
                    _ => Load::Ashlar,
                };
                paint::pile(&mut im, 8, h - 2, 14, 9, load, seed);
                let (trade, act) = if step == 2 { (Trade::Setter, Act::Work) } else { (Trade::Carpenter, Act::Work) };
                hands::stamp(&mut im, w / 2 + 6, rows[0].max(8), trade, act, 3, false);
            }
        }
    }
    im
}

/// A building not yet drawn: a plain house in its footprint.
pub(crate) fn placeholder(c: &mut Ctx) {
    let (w, h) = (c.w, c.h);
    paint::earth(&mut c.im, w as f32 / 2.0, h as f32 - 8.0, w as f32 * 0.45, 8.0, c.seed);
    let wall = (h / 2).max(8);
    paint::stone(&mut c.im, 3, h - wall - 3, w - 6, wall, c.seed);
    paint::roof(&mut c.im, 1, 2, w - 2, h - wall - 4, crate::render::overworld::material::ramp::SLATE, c.seed);
}
