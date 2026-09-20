//! The cockpit miniworld — a gamified systems view for the artifacts pane.
//!
//! A tiny medieval Arthurian castle-town on a procedurally generated island,
//! built from `terrain-forge` noise and seeded by the workspace path, so every
//! project gets its own stable little realm. The agent's real internal workings
//! drive it like a tamagotchi-Zelda hybrid: the knight rides to the Scriptorium
//! when files are read, hammers at the Smithy on writes and builds, seals
//! scrolls at the Rookery on `git commit`, carries dispatches out through the
//! Gatehouse on `git push` (riding to the realm), holds council at the Round
//! Table on `delegate`, and prays in the Chapel when memory is deposited or the
//! context compacts. Quota storms literally rain on the town.
//!
//! The Scryglass world is always on: deterministic, protocol-free, and
//! SSH-safe. Travel and quests use the true-3D Dotmax world. Explicit entry
//! opens an ambient room plate; leaving restores Dotmax.
//!
//! Pure display: nothing here feeds back into the model or the turn.

mod activity;
mod adventure;
mod ambient;
mod camera;
pub(crate) mod cinematics;
mod hud;
mod interiors;
pub(crate) mod life;
mod raycast;
mod region_walk;
mod ride;
mod room_plates;
pub(crate) mod world3d;
pub(crate) mod world_camera;

#[cfg(test)]
use activity::ClassifiedActivity;
pub(crate) use activity::{ActiveWork, RealmActivity, classify_tool_activity};
use activity::{pulse_operation, tool_leaf};
pub(crate) use adventure::{AdventureEvent, LoopMirror, Quest, ToolMix};
#[cfg(test)]
pub(crate) use adventure::{LoopKind, Region};
use camera::{Camera, CameraMode, SETTLE_TICKS, Z_CLOSE, Z_WIDE};
pub(crate) use cinematics::warm_assets as warm_cinematic_assets;

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};

#[cfg(test)]
mod world_scene_tax {
    use std::cell::Cell;

    thread_local! {
        static RIDE_COMPOSES: Cell<u64> = const { Cell::new(0) };
    }

    pub(super) fn note_ride_compose() {
        RIDE_COMPOSES.with(|count| count.set(count.get().saturating_add(1)));
    }

    pub(crate) fn take_ride_compose_count() -> u64 {
        RIDE_COMPOSES.with(|count| count.replace(0))
    }
}

#[cfg(test)]
pub(crate) use world_scene_tax::take_ride_compose_count;

use crate::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};

use dotmax::Color as DotColor;

mod budget;
mod seed;
mod terrain;
pub(crate) use budget::{
    fit_spans_to_cells, health_bar, remaining_fraction, spans_cell_width, world_budget_status,
};
#[cfg(test)]
use ratatui::style::{Color, Style};
#[cfg(test)]
use ratatui::text::{Line, Span, Text};
pub(crate) use seed::{district_banner_color, district_hash, fnv1a, town_name};
#[cfg(test)]
pub(super) use terrain::on_road_band;
use terrain_forge::noise::{Fbm, NoiseExt, NoiseSource, Perlin};

pub(crate) const WORLD_W: usize = 64;
pub(crate) const WORLD_H: usize = 26;

/// Stable realm regions shared by landmarks and named districts.
const DISTRICT_FRACTIONS: [(f64, f64); 8] = crate::identity::district_fractions();

// The townsfolk are drawn in plain black-and-white: a white outline + face on
// the dark silhouette. Only the world around them (terrain, shop signs,
// sparkles) carries color — the characters themselves stay monochrome so they
// read as little ink sketches against the colored map.

#[cfg_attr(not(test), allow(dead_code))]
fn cell_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

#[cfg_attr(not(test), allow(dead_code))]
fn fit_cells(text: &str, max: usize) -> String {
    if cell_width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let keep = max.saturating_sub(1);
    let mut fitted = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(width) > keep {
            break;
        }
        fitted.push(ch);
        used += width;
    }
    fitted.push('…');
    fitted
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Biome {
    DeepWater,
    Water,
    Sand,
    Grass,
    Forest,
    Hill,
    Peak,
    Path,
}

/// Places the agent's real activities live in. Each one is a landmark on the
/// island; tool traffic sends the knight riding between them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(crate) enum Building {
    /// The keep — where the knight rests between turns.
    Keep,
    /// Gatehouse on the west shore: the link to the outside world (git
    /// remotes, HTTP clubs). Dispatches leave the realm through here.
    Gatehouse,
    /// Rookery: `git commit` seals scrolls here before they fly out.
    Rookery,
    /// Scriptorium: file reads, greps, searches.
    Scriptorium,
    /// Smithy: writes, edits, builds, tests.
    Smithy,
    /// Hillside chapel: memory deposits, recall, compaction vigils.
    Chapel,
    /// Round Table: council/delegate sessions with other agents.
    RoundTable,
    /// Observatory on the northern hill: the science synthesis bench. Literature
    /// fan-outs and research light its dome — the town's window on the heavens.
    Observatory,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LoopBudgetSnapshot {
    pub(crate) iteration: usize,
    pub(crate) max_iters: usize,
    pub(crate) tokens_spent: usize,
    pub(crate) token_budget: usize,
    pub(crate) elapsed_secs: u64,
    pub(crate) deadline_secs: u64,
}

impl Building {
    pub(crate) fn label(self) -> &'static str {
        crate::identity::landmark(self).display_name
    }

    pub(crate) fn glyph(self) -> char {
        ['⌂', '∏', '♜', '▤', '⚒', '†', '◉', '✦'][self as usize]
    }

    fn arrive_activity(self) -> &'static str {
        crate::identity::landmark(self).arrive_activity
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KnightPose {
    Riding,
    Working,
    Resting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct District {
    pub(crate) name: String,
    pub(crate) anchor: (i32, i32),
    pub(crate) banner_color: (u8, u8, u8),
}

pub(crate) struct World {
    /// Hash of the workspace path — the world's stable RNG seed, reused to
    /// scatter the scenery library the same way every session.
    seed: u64,
    tiles: Vec<Biome>,
    /// Continuous terrain fields shared by navigation and the retained ride.
    elevation: Fbm<Perlin>,
    moisture: Fbm<Perlin>,
    /// Static elevation at each tile centre (`x + 0.5`, `y + 0.5`). Travel
    /// cinematics remarch the same window often, so they read this cache
    /// instead of evaluating four-octave FBM for every open cell each time.
    tile_elevation: Vec<f32>,
    buildings: Vec<(Building, (usize, usize))>,
    town_name: String,
    /// Opt-in repo districts. Empty preserves the historical render exactly.
    districts: Vec<District>,
    avatar: (usize, usize),
    target: Building,
    /// Opt-in authored room currently shown by the shared raycaster.
    interior: Option<Building>,
    /// Remaining tiles in the current journey, excluding the avatar's tile.
    route: VecDeque<(usize, usize)>,
    /// Destination for which `route` was computed. A target change or a
    /// desynchronized next step causes a lazy recompute in `tick`.
    route_goal: Option<(usize, usize)>,
    active_work: BTreeMap<ToolEventId, ActiveWork>,
    recent_work: VecDeque<ActiveWork>,
    completed_event_ids: BTreeSet<ToolEventId>,
    completed_event_order: VecDeque<ToolEventId>,
    event_seq: u64,
    event_diagnostics: u64,
    memory_health: crate::memory_store::MemoryHealth,
    atlas_health: crate::atlas::AtlasHealth,
    atlas_review_count: usize,
    clerk_health: crate::atlas_clerk::ClerkHealth,
    clerk_queue_depth: usize,
    resource_conflicts: usize,
    carrying_mail: bool,
    activity: String,
    tick: u64,
    storm_until: u64,
    sparkle_until: u64,
    recent_errors: u32,
    /// The travel / close camera. At `Z_WIDE` the render follows the integer
    /// avatar; `camera` drives the eased close view.
    camera: Camera,
    /// Sub-tile visual position of the knight, eased toward the integer
    /// `avatar` every tick. Render-only — all game logic uses `avatar`.
    avatar_vis: (f32, f32),
    /// Consecutive ticks the knight has sat arrived at its target — the auto
    /// camera's punch-in counter (reset the instant a new target is assigned).
    settle_ticks: u32,
    /// Which way the near-view knight sprite faces, from its last x step.
    facing_left: bool,
    /// First-person travel camera heading (radians, world space), eased
    /// toward the direction of motion so Manhattan steps read as smooth
    /// turns. Render-only, like `avatar_vis`.
    travel_heading: f32,
    /// Consecutive ticks the current journey has been under way — drives the
    /// plate→3D departure dissolve (mirror of `settle_ticks`).
    travel_ticks: u32,
    /// The last location the knight actually arrived at: the plate the next
    /// departure dissolves from.
    plate_from: Building,

    // ─── the quintain: the self-improvement loop, absorbed into the town ─────
    /// A fixed spot on land where the loop lives. The knight returns here to
    /// "tilt" between rounds; the wheel turns while a loop runs.
    quintain: (usize, usize),
    /// True while a loop is running — the quintain turns and the knight rests at
    /// it between rounds instead of at the keep.
    loop_active: bool,
    /// Highest loop iteration seen, so a new round can sparkle a finding.
    loop_iteration: usize,
    /// Last loop budget snapshot shown as health bars in the world status line.
    loop_budget: Option<LoopBudgetSnapshot>,
    /// Last agitation state (escalation / stall / awaiting SOTA) — storms fire
    /// on the rising edge, not every frame.
    loop_agitated: bool,

    // ─── the adventure: the loop as a quest through the regions (Z1) ───────
    /// Pure quest state — region, danger, loot, banners, party — driven by
    /// `note_adventure`. Session state; never persisted in world-rewards.
    quest: Quest,
    /// Rolling classification of the last tool calls, feeding
    /// `adventure::loop_kind_for` when a loop starts.
    tool_mix: ToolMix,

    // ─── progression: one truthful, display-only currency ───────────────────
    /// The sole visible progression currency. Renown never decreases and only
    /// the four locked, attributable events may credit it.
    renown: u64,
    /// Count of completion-sufficient verifier wins, persisted for history.
    verified_wins: u64,
    /// Legacy rollback fields. They are loaded and saved unchanged, but never
    /// shown or mutated by the v2 progression system.
    xp: u64,
    coins: u64,
    /// Consecutive green turns remain session-local atmosphere only.
    streak: u32,
    turn_epoch: u64,
    turn_verified_awarded: bool,
    turn_research_awarded: bool,
    credited_renown_ids: BTreeSet<String>,
    credited_renown_order: VecDeque<String>,
    /// Consecutive failed turns — two or more make it rain, and the first
    /// green after a gray run hangs a rainbow over the town.
    fail_run: u32,
    /// Tick until which the recovery rainbow shows in the weather.
    rainbow_until: u64,
    /// Fireworks burst over the town until this tick (victories, level-ups,
    /// discoveries) — the payout made loud.
    firework_until: u64,
    /// Tick the fireworks were lit (keys the burst expansion phase).
    firework_from: u64,
    /// A short renown readout flashed at the end of the status line in gold.
    gain_note: String,
    gain_until: u64,
    /// Where progression persists (best-effort JSON). `None` outside a real
    /// workspace session, so unit-test worlds never touch the disk.
    rewards_path: Option<std::path::PathBuf>,
    /// The hearth — the slow living state (light/warmth/grain + day clock).
    /// Session moods stay on the World; the hearth persists beside renown.
    hearth: crate::hearth::HearthState,
    /// Tool successes banked since the last hearth beat (fed in as `tool_wins`).
    hearth_pending_wins: u32,
    /// Consecutive truthful red tool outcomes. Kept deliberately coarse: the
    /// sky only distinguishes calm, gathering, and raining pressure.
    outcome_err_streak: u8,
    /// Hearth beat through which the first verified green after a red run
    /// shows the bounded clearing sky.
    clearing_until_beat: u64,
    /// Read-only mirror of a completion-class lifecycle ceremony. The app
    /// updates this from the same timed ceremony object that owns the overlay.
    completion_ceremony_active: bool,
    /// The village — the fleet mirrored into the town (see `village.rs`).
    /// `None` (tests, `ANGEL_VILLAGE=0`) renders the map exactly as before.
    village: Option<crate::village::Village>,
    /// Burst origin for the current fireworks besides the keep — a promotion
    /// celebrates over the forge instead of the knight's target. Reset by
    /// `light_fireworks`; only read while the show runs.
    firework_focus: Option<(usize, usize)>,
    /// Memoized close-camera terrain: the zoomed renderer samples the noise
    /// fields at sub-tile world points every frame, but the island never
    /// changes — answers are cached on a 1/16-tile lattice (well under the
    /// braille dot pitch at Z_CLOSE, so quantization is invisible). RefCell
    /// because `render` is `&self`; bounded so a long pan can't grow it
    /// unbounded while the retained ride samples continuous terrain.
    #[cfg_attr(not(test), allow(dead_code))]
    close_cache: std::cell::RefCell<std::collections::HashMap<(i16, i16), Biome>>,
    // ─── the muster: fan-out stages rendered as an army on the field ────────
    /// Sequence of the last stage snapshot consumed from `agentviz`, so a new
    /// wave/panel re-forms the ranks exactly once.
    muster_seq: u64,
    /// One unit per fan-out seat: eased visual position, assigned slot, glyph
    /// and ink by stage kind (soldiers/magistrates/sentries…). Empty = no army.
    muster: Vec<MusterUnit>,

    /// Bumped by every tile write so a carved path invalidates the memo.
    terrain_epoch: u64,
    /// The braille ride's last frame. World animation and operator camera input
    /// have separate keys: relaxed live-turn pacing may hold an older world
    /// frame, but yaw, pitch, FOV, or pane geometry must take effect immediately.
    ride_cache: RefCell<Option<RideCacheEntry>>,
    /// Highest growth tier already announced this session. Boot-time worlds
    /// adopt their persisted tier silently (set in `for_workspace`), so only
    /// fresh crossings celebrate.
    growth_announced: u32,
    /// The growth build plan for the current tier plus its next construction
    /// site, memoized — the island and its landmarks never move, so the tier
    /// is the whole key (`life.rs`).
    growth_cache: RefCell<Option<(u32, life::GrowthLayout)>>,
}

#[derive(Clone, Debug, Default)]
struct LoadedRewards {
    renown: u64,
    verified_wins: u64,
    xp: u64,
    coins: u64,
    hearth: crate::hearth::HearthState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RideViewKey {
    cells_w: usize,
    cells_h: usize,
    yaw: u32,
    pitch: u32,
    fov: u32,
}

struct RideCacheEntry {
    world_key: u64,
    view_key: RideViewKey,
    rendered_at: u64,
    image: std::sync::Arc<crate::terminal_art::ColoredBrailleImage>,
}

#[derive(Clone, Copy, Debug)]
struct MusterUnit {
    vis: (f32, f32),
    slot: (usize, usize),
    glyph: char,
    ink: DotColor,
}

impl World {
    /// Operator-selected map travel is display-only. It changes the visual
    /// destination and lets the existing world tick build the route; no tool,
    /// reward, model, or execution state is affected.
    pub(crate) fn cycle_landmark(&mut self, delta: i32) {
        const LANDMARKS: [Building; 8] = [
            Building::Gatehouse,
            Building::Rookery,
            Building::Observatory,
            Building::Chapel,
            Building::Keep,
            Building::Scriptorium,
            Building::Smithy,
            Building::RoundTable,
        ];
        let current = LANDMARKS
            .iter()
            .position(|&building| building == self.target)
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(LANDMARKS.len() as i32) as usize;
        self.select_landmark(LANDMARKS[next]);
    }

    pub(crate) fn select_landmark(&mut self, building: Building) {
        self.target = building;
        self.carrying_mail = false;
        self.interior = None;
        self.activity = format!("map route → {}", building.label());
    }

    /// Deterministic world from a seed (hash of the workspace path): same
    /// project, same island, every session.
    pub(crate) fn new(seed: u64) -> Self {
        let mut world = World {
            seed,
            tiles: Vec::new(),
            elevation: Perlin::new(seed).fbm(4, 2.0, 0.5),
            moisture: Perlin::new(seed ^ 0x9e37_79b9_7f4a_7c15).fbm(3, 2.0, 0.5),
            tile_elevation: Vec::new(),
            buildings: Vec::new(),
            town_name: town_name(seed),
            districts: Vec::new(),
            avatar: (WORLD_W / 2, WORLD_H / 2),
            target: Building::Keep,
            interior: None,
            route: VecDeque::new(),
            route_goal: None,
            active_work: BTreeMap::new(),
            recent_work: VecDeque::new(),
            completed_event_ids: BTreeSet::new(),
            completed_event_order: VecDeque::new(),
            event_seq: 0,
            event_diagnostics: 0,
            memory_health: crate::memory_store::MemoryHealth::Disabled,
            atlas_health: crate::atlas::AtlasHealth::Disabled,
            atlas_review_count: 0,
            clerk_health: crate::atlas_clerk::ClerkHealth::Idle,
            clerk_queue_depth: 0,
            resource_conflicts: 0,
            carrying_mail: false,
            activity: "resting in the keep".to_string(),
            tick: 0,
            storm_until: 0,
            sparkle_until: 0,
            recent_errors: 0,
            camera: Camera::wide(),
            avatar_vis: (0.0, 0.0),
            settle_ticks: 0,
            facing_left: false,
            travel_heading: 0.0,
            travel_ticks: 0,
            plate_from: Building::Keep,
            quintain: (WORLD_W / 2, WORLD_H / 2),
            loop_active: false,
            loop_iteration: 0,
            loop_budget: None,
            loop_agitated: false,
            quest: Quest::idle(),
            tool_mix: ToolMix::default(),
            renown: 0,
            verified_wins: 0,
            xp: 0,
            coins: 0,
            streak: 0,
            turn_epoch: 0,
            turn_verified_awarded: false,
            turn_research_awarded: false,
            credited_renown_ids: BTreeSet::new(),
            credited_renown_order: VecDeque::new(),
            fail_run: 0,
            rainbow_until: 0,
            firework_until: 0,
            firework_from: 0,
            gain_note: String::new(),
            gain_until: 0,
            rewards_path: None,
            hearth: crate::hearth::HearthState::default(),
            hearth_pending_wins: 0,
            outcome_err_streak: 0,
            clearing_until_beat: 0,
            completion_ceremony_active: false,
            village: None,
            firework_focus: None,
            close_cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            muster_seq: 0,
            muster: Vec::new(),
            ride_cache: RefCell::new(None),
            terrain_epoch: 0,
            growth_announced: 0,
            growth_cache: RefCell::new(None),
        };
        world.tiles = (0..WORLD_H)
            .flat_map(|y| (0..WORLD_W).map(move |x| (x, y)))
            .map(|(x, y)| world.natural_biome_at(x as f64, y as f64))
            .collect();
        world.tile_elevation = (0..WORLD_H)
            .flat_map(|y| (0..WORLD_W).map(move |x| (x, y)))
            .map(|(x, y)| world.elevation_at(x as f64 + 0.5, y as f64 + 0.5) as f32)
            .collect();
        world.place_buildings();
        world.carve_paths();
        world.avatar = world.building_pos(Building::Keep);
        world.avatar_vis = (world.avatar.0 as f32, world.avatar.1 as f32);
        world.camera.look_at(world.avatar);
        // The quintain sits just north of the keep, snapped to open land clear
        // of the landmarks — a stable spot the loop calls home.
        let taken: Vec<(usize, usize)> = world.buildings.iter().map(|&(_, p)| p).collect();
        world.quintain = world.nearest_land(WORLD_W as f64 * 0.5, WORLD_H as f64 * 0.4, &taken);
        world
    }

    /// A wooded mountain valley, continuous in world coordinates. The former
    /// radial island falloff made every settlement a distant coastal icon.
    /// A narrow stream crosses the valley; carved roads bridge it as before.
    fn elevation_at(&self, x: f64, y: f64) -> f64 {
        let stream = WORLD_W as f64 * 0.31 + (y * 0.28).sin() * 2.5;
        let bank_distance = (x - stream).abs();
        if bank_distance < 0.65 {
            return -0.07 + bank_distance * 0.1;
        }
        let ridge = ((y / WORLD_H as f64 - 0.5).abs() * 2.0).powi(2);
        0.19 + ridge * 0.43 + self.elevation.sample(x * 0.09, y * 0.17) * 0.20
    }

    /// Biome from the continuous noise fields (before paths are carved).
    /// Fractional coordinates give the braille renderer smooth coastlines.
    fn natural_biome_at(&self, x: f64, y: f64) -> Biome {
        let e = self.elevation_at(x, y);
        match e {
            e if e < -0.12 => Biome::DeepWater,
            e if e < 0.02 => Biome::Water,
            e if e < 0.07 => Biome::Sand,
            e if e < 0.42 => {
                if self.moisture.sample(x * 0.13, y * 0.23) > -0.08 {
                    Biome::Forest
                } else {
                    Biome::Grass
                }
            }
            e if e < 0.58 => Biome::Hill,
            _ => Biome::Peak,
        }
    }

    /// Seed a world from the workspace path so each project keeps its island —
    /// and restore its versioned, display-only progression state.
    /// Only real sessions get a rewards file; `World::new` stays disk-free.
    pub(crate) fn for_workspace(workspace: &std::path::Path) -> Self {
        let identity = crate::workspace_store::repo_identity(workspace);
        let mut world = Self::new(fnv1a(identity.root.to_string_lossy().as_bytes()));
        world.rewards_path = std::env::var_os("HOME").map(|h| {
            std::path::Path::new(&h)
                .join(".angel0")
                .join("world-rewards")
                .join(format!("{}.json", identity.key))
        });
        if let Some(p) = &world.rewards_path {
            let loaded = Self::load_rewards(p);
            world.renown = loaded.renown;
            world.verified_wins = loaded.verified_wins;
            world.xp = loaded.xp;
            world.coins = loaded.coins;
            world.hearth = loaded.hearth;
            world.hearth.prosperity = world.renown.min(999) as u32;
        }
        // History loads silently: a town that already grew does not re-announce
        // its structures — only crossings from here on celebrate.
        world.growth_announced = life::town_tier(world.renown.min(u64::from(u32::MAX)) as u32);
        world
    }

    fn at(&self, x: usize, y: usize) -> Biome {
        self.tiles[y * WORLD_W + x]
    }

    /// Close-camera terrain lookup, memoized on a 1/16-tile lattice. The
    /// answer is `natural_biome_at` evaluated at the lattice point's centre —
    /// finer than the braille dot pitch at `Z_CLOSE`, so the quantization
    /// never shows — cached because the 4-octave FBM is the whole cost of the
    /// zoomed render. Bounded: a runaway pan clears and starts over.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn close_biome_at(&self, wx: f32, wy: f32) -> Biome {
        const CAP: usize = 120_000;
        let key = ((wx * 16.0).floor() as i16, (wy * 16.0).floor() as i16);
        let mut cache = self.close_cache.borrow_mut();
        if let Some(&b) = cache.get(&key) {
            return b;
        }
        let b = self.natural_biome_at(
            (f64::from(key.0) + 0.5) / 16.0,
            (f64::from(key.1) + 0.5) / 16.0,
        );
        if cache.len() >= CAP {
            cache.clear();
        }
        cache.insert(key, b);
        b
    }

    /// The discrete tile at a fractional world point, or `DeepWater` off-map.
    /// The zoomed sampler uses it to find carved roads — the noise fields know
    /// nothing about the paths baked into `tiles`.
    fn tile_at_world(&self, wx: f32, wy: f32) -> Biome {
        let (x, y) = (wx.floor(), wy.floor());
        if x < 0.0 || y < 0.0 || x >= WORLD_W as f32 || y >= WORLD_H as f32 {
            return Biome::DeepWater;
        }
        self.at(x as usize, y as usize)
    }

    fn set(&mut self, x: usize, y: usize, b: Biome) {
        self.tiles[y * WORLD_W + x] = b;
        self.terrain_epoch += 1;
    }

    /// Integer cost to enter a tile while travelling. Roads are deliberately
    /// cheapest; water is blocked, and hills remain legal only as a last resort.
    fn route_step_cost(&self, x: usize, y: usize) -> Option<u32> {
        let elevation = (self.tile_elevation[y * WORLD_W + x] * 40.0) as u32;
        match self.at(x, y) {
            Biome::Path => Some(10),
            Biome::Grass | Biome::Sand => Some(30 + elevation),
            Biome::Forest => Some(45 + elevation),
            Biome::Hill => Some(300),
            Biome::Peak => Some(600),
            Biome::Water | Biome::DeepWater => None,
        }
    }

    /// Deterministic Dijkstra route, excluding `start` and including `goal`.
    /// Heap ordering is exactly `(cost, y, x)` via `Reverse`; all grids are
    /// fixed row-major vectors, so no hash iteration can affect the result.
    fn least_cost_route(
        &self,
        start: (usize, usize),
        goal: (usize, usize),
    ) -> Option<Vec<(usize, usize)>> {
        if start == goal {
            return Some(Vec::new());
        }
        if start.0 >= WORLD_W || start.1 >= WORLD_H || goal.0 >= WORLD_W || goal.1 >= WORLD_H {
            return None;
        }

        let tile_count = WORLD_W * WORLD_H;
        let start_index = start.1 * WORLD_W + start.0;
        let goal_index = goal.1 * WORLD_W + goal.0;
        let mut distance = vec![u32::MAX; tile_count];
        let mut visited = vec![false; tile_count];
        let mut came_from = vec![None; tile_count];
        let mut frontier = BinaryHeap::new();
        distance[start_index] = 0;
        frontier.push(Reverse((0_u32, start.1, start.0)));

        while let Some(Reverse((cost, y, x))) = frontier.pop() {
            let index = y * WORLD_W + x;
            if visited[index] {
                continue;
            }
            visited[index] = true;
            if index == goal_index {
                break;
            }

            for (dx, dy) in [(0_i32, -1_i32), (-1, 0), (1, 0), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= WORLD_W as i32 || ny >= WORLD_H as i32 {
                    continue;
                }
                let next = (nx as usize, ny as usize);
                if next != goal && self.buildings.iter().any(|&(_, pos)| pos == next) {
                    continue;
                }
                let Some(step_cost) = self.route_step_cost(next.0, next.1) else {
                    continue;
                };
                let next_index = next.1 * WORLD_W + next.0;
                let next_cost = cost.saturating_add(step_cost);
                if next_cost < distance[next_index] {
                    distance[next_index] = next_cost;
                    came_from[next_index] = Some(index);
                    frontier.push(Reverse((next_cost, next.1, next.0)));
                }
            }
        }

        if !visited[goal_index] {
            return None;
        }
        let mut route = Vec::new();
        let mut index = goal_index;
        while index != start_index {
            route.push((index % WORLD_W, index / WORLD_W));
            index = came_from[index]?;
        }
        route.reverse();
        Some(route)
    }

    /// Force the camera zoom, bypassing the auto state machine — the terrain
    /// sampler and clamping can be exercised at close range before M3 exists.
    #[cfg(test)]
    pub(crate) fn set_zoom_for_test(&mut self, zoom: f32) {
        self.camera.zoom = zoom;
    }

    /// Sim clock used by advance() scenery ticks — test-only observability.
    #[cfg(test)]
    pub(crate) fn sim_tick_for_test(&self) -> u64 {
        self.tick
    }

    fn is_land(&self, x: usize, y: usize) -> bool {
        matches!(self.at(x, y), Biome::Grass | Biome::Forest | Biome::Sand)
    }

    /// Nearest land tile to an anchor point that isn't already taken.
    fn nearest_land(&self, ax: f64, ay: f64, taken: &[(usize, usize)]) -> (usize, usize) {
        let mut best = (WORLD_W / 2, WORLD_H / 2);
        let mut best_d = f64::MAX;
        for y in 1..WORLD_H - 1 {
            for x in 1..WORLD_W - 1 {
                if !self.is_land(x, y) || taken.contains(&(x, y)) {
                    continue;
                }
                let d = (x as f64 - ax).powi(2) + ((y as f64 - ay) * 2.0).powi(2);
                if d < best_d {
                    best_d = d;
                    best = (x, y);
                }
            }
        }
        best
    }

    fn place_buildings(&mut self) {
        let w = WORLD_W as f64;
        let h = WORLD_H as f64;
        // Anchor each building to a district, then snap to real land.
        let kinds = [
            Building::Keep,
            Building::Gatehouse,
            Building::Rookery,
            Building::Scriptorium,
            Building::Smithy,
            Building::Chapel,
            Building::RoundTable,
            Building::Observatory,
        ];
        let mut taken: Vec<(usize, usize)> = Vec::new();
        for (kind, (fx, fy)) in kinds.into_iter().zip(DISTRICT_FRACTIONS) {
            let pos = self.nearest_land(w * fx, h * fy, &taken);
            taken.push(pos);
            self.buildings.push((kind, pos));
        }
    }

    /// Roads from every landmark back to the keep — stepped lines that bridge
    /// water so the whole town is visibly connected (very Zelda overworld).
    fn carve_paths(&mut self) {
        let home = self.building_pos(Building::Keep);
        let stops: Vec<(usize, usize)> = self
            .buildings
            .iter()
            .filter(|(k, _)| *k != Building::Keep)
            .map(|&(_, p)| p)
            .collect();
        for stop in stops {
            let (mut x, mut y) = (stop.0 as i32, stop.1 as i32);
            let (hx, hy) = (home.0 as i32, home.1 as i32);
            while (x, y) != (hx, hy) {
                // Step along the larger remaining axis first: gentle L-bends.
                let dx = hx - x;
                let dy = hy - y;
                if dx.abs() >= dy.abs() {
                    x += dx.signum();
                } else {
                    y += dy.signum();
                }
                let (ux, uy) = (x as usize, y as usize);
                if (ux, uy) != home && !self.buildings.iter().any(|&(_, p)| p == (ux, uy)) {
                    self.set(ux, uy, Biome::Path);
                }
            }
        }
    }

    pub(crate) fn building_pos(&self, kind: Building) -> (usize, usize) {
        self.buildings
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|&(_, p)| p)
            .unwrap_or((WORLD_W / 2, WORLD_H / 2))
    }

    pub(crate) fn town_name(&self) -> &str {
        &self.town_name
    }

    /// Name repo districts without consulting the filesystem. Names are
    /// sorted; overflow is represented by one eighth "Outlands" district.
    pub(crate) fn enable_districts(&mut self, mut names: Vec<String>) {
        names.sort();
        if names.len() > DISTRICT_FRACTIONS.len() {
            names.truncate(DISTRICT_FRACTIONS.len() - 1);
            names.push("Outlands".to_string());
        }

        let mut taken: Vec<(usize, usize)> = self.buildings.iter().map(|&(_, p)| p).collect();
        taken.push(self.quintain);
        let mut regions_taken = [false; DISTRICT_FRACTIONS.len()];
        let districts = names
            .into_iter()
            .map(|name| {
                let hash = district_hash(self.seed, &name);
                let preferred = hash as usize % DISTRICT_FRACTIONS.len();
                let region = (0..DISTRICT_FRACTIONS.len())
                    .map(|step| (preferred + step) % DISTRICT_FRACTIONS.len())
                    .find(|&index| !regions_taken[index])
                    .expect("at most eight districts");
                regions_taken[region] = true;
                let (fx, fy) = DISTRICT_FRACTIONS[region];
                let anchor = self.nearest_land(WORLD_W as f64 * fx, WORLD_H as f64 * fy, &taken);
                taken.push(anchor);
                District {
                    name,
                    anchor: (anchor.0 as i32, anchor.1 as i32),
                    banner_color: district_banner_color(hash),
                }
            })
            .collect();
        self.districts = districts;
    }

    #[cfg(test)]
    #[allow(clippy::type_complexity)] // (name, cell, rgb) test signature; alias adds no clarity
    pub(crate) fn district_signature(&self) -> Vec<(String, (i32, i32), (u8, u8, u8))> {
        self.districts
            .iter()
            .map(|district| {
                (
                    district.name.clone(),
                    district.anchor,
                    district.banner_color,
                )
            })
            .collect()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn district_at(&self, point: (f32, f32)) -> Option<&District> {
        self.districts.iter().min_by(|a, b| {
            let distance = |district: &District| {
                let dx = point.0 - district.anchor.0 as f32;
                let dy = (point.1 - district.anchor.1 as f32) * 2.0;
                dx * dx + dy * dy
            };
            distance(a).total_cmp(&distance(b))
        })
    }

    /// Nearest named ward intersected by a route segment. Distance is measured
    /// in the same display-space metric as the map's district regions.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn district_on_route(
        &self,
        start: (f32, f32),
        end: (f32, f32),
    ) -> Option<&District> {
        const REGION_RADIUS_SQ: f32 = 7.0 * 7.0;
        let start = (start.0, start.1 * 2.0);
        let end = (end.0, end.1 * 2.0);
        let segment = (end.0 - start.0, end.1 - start.1);
        let length_sq = segment.0 * segment.0 + segment.1 * segment.1;
        self.districts
            .iter()
            .filter_map(|district| {
                let anchor = (district.anchor.0 as f32, district.anchor.1 as f32 * 2.0);
                let offset = (anchor.0 - start.0, anchor.1 - start.1);
                let along = if length_sq > f32::EPSILON {
                    ((offset.0 * segment.0 + offset.1 * segment.1) / length_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let closest = (start.0 + segment.0 * along, start.1 + segment.1 * along);
                let dx = anchor.0 - closest.0;
                let dy = anchor.1 - closest.1;
                let distance_sq = dx * dx + dy * dy;
                (distance_sq <= REGION_RADIUS_SQ).then_some((distance_sq, district))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, district)| district)
    }

    pub(crate) fn destination(&self) -> Building {
        self.target
    }

    pub(crate) fn has_authored_interior(&self) -> bool {
        interiors::supports(self.target)
    }

    pub(crate) fn inside_interior(&self) -> bool {
        self.interior.is_some()
    }

    pub(crate) fn interior_building(&self) -> Option<Building> {
        self.interior
    }

    pub(crate) fn enter_interior(&mut self) -> bool {
        if self.avatar == self.dest()
            && self.avatar_vis_settled()
            && interiors::supports(self.target)
        {
            self.interior = Some(self.target);
            true
        } else {
            false
        }
    }

    pub(crate) fn leave_interior(&mut self) -> bool {
        self.interior.take().is_some()
    }

    #[cfg(test)]
    pub(crate) fn settle_at_for_test(&mut self, building: Building) {
        self.target = building;
        let dest = self.dest();
        self.avatar = dest;
        self.avatar_vis = (dest.0 as f32, dest.1 as f32);
        self.route.clear();
        self.settle_ticks = cinematics::ARRIVAL_HOLD_TICKS;
    }

    #[cfg(test)]
    pub(crate) fn weather_state_for_test(
        &mut self,
        clock: life::Weather,
        err_streak: u8,
        clearing_beats: u64,
    ) {
        self.hearth.beats = (0..20_000)
            .find(|&beat| life::weather_for_beats(beat, self.seed) == clock)
            .expect("requested clock weather has a beat");
        self.outcome_err_streak = err_streak;
        self.clearing_until_beat = self.hearth.beats.saturating_add(clearing_beats);
    }

    pub(crate) fn renown(&self) -> u64 {
        self.renown
    }

    // ─── the gamification: real internal events → world happenings ──────────

    /// A selected concept is world knowledge, not a model tool call. Send the
    /// knight toward the Scriptorium without creating an ActiveWork record or
    /// awarding renown; closing the teaching window reveals the same physical
    /// journey and authored library interior.
    pub(crate) fn begin_teaching_lesson(&mut self, term: &str) {
        self.target = Building::Scriptorium;
        self.carrying_mail = false;
        self.activity = format!(
            "world lesson · {} → {}",
            term.chars().take(48).collect::<String>(),
            Building::Scriptorium.label()
        );
    }

    /// A tool call starts activity at one classified landmark. It never pays a
    /// reward; only the correlated result may produce an outcome or progression.
    pub(crate) fn note_tool_call_event(&mut self, id: ToolEventId, name: &str, args_summary: &str) {
        if self.active_work.contains_key(&id) || self.completed_event_ids.contains(&id) {
            self.event_diagnostics = self.event_diagnostics.saturating_add(1);
            return;
        }
        let classified = classify_tool_activity(name, args_summary);
        // Z1: tool traffic feeds the adventure's rolling mix (for
        // `loop_kind_for`) and the quest's Tool event; the town-landmark walk
        // below is unchanged.
        self.tool_mix.push(classified.building, classified.activity);
        self.note_adventure(AdventureEvent::Tool(
            classified.building,
            classified.activity,
        ));
        self.event_seq = self.event_seq.saturating_add(1);
        let literal = if args_summary.trim().is_empty() {
            name.to_string()
        } else {
            format!("{name} · {args_summary}")
        }
        .chars()
        .take(240)
        .collect::<String>();
        let work = ActiveWork {
            id: id.clone(),
            operation: name.chars().take(240).collect(),
            pulse_operation: pulse_operation(name, args_summary),
            literal: literal.clone(),
            landmark: classified.building,
            activity: classified.activity,
            summary: String::new(),
            outcome: None,
            seq: self.event_seq,
        };
        self.target = classified.building;
        self.carrying_mail = classified.building == Building::Gatehouse;
        self.activity = format!("{literal} → {}", classified.building.label());
        self.active_work.insert(id, work);
    }

    /// Mirror the currently visible completion ceremony. Callers derive this
    /// from the overlay's own timed lifecycle; World never extends it.
    pub(crate) fn set_completion_ceremony_active(&mut self, active: bool) {
        self.completion_ceremony_active = active;
    }

    /// Complete exactly one active call. Unknown and duplicate ids are retained
    /// only as neutral diagnostics and can never sparkle or pay progression.
    pub(crate) fn note_tool_result_event(
        &mut self,
        id: &ToolEventId,
        name: &str,
        summary: &str,
        outcome: ToolOutcome,
    ) {
        if self.completed_event_ids.contains(id) {
            self.event_diagnostics = self.event_diagnostics.saturating_add(1);
            return;
        }
        let Some(mut work) = self.active_work.remove(id) else {
            self.event_diagnostics = self.event_diagnostics.saturating_add(1);
            return;
        };
        if tool_leaf(name) != tool_leaf(&work.operation) {
            self.event_diagnostics = self.event_diagnostics.saturating_add(1);
        }
        work.summary = summary.chars().take(240).collect();
        work.outcome = Some(outcome);
        self.completed_event_ids.insert(id.clone());
        self.completed_event_order.push_back(id.clone());
        while self.completed_event_order.len() > 512 {
            if let Some(expired) = self.completed_event_order.pop_front() {
                self.completed_event_ids.remove(&expired);
            }
        }

        if outcome.attributable_success() {
            self.recent_errors = self.recent_errors.saturating_sub(1);
            self.sparkle_until = self.tick + 14;
            self.hearth_pending_wins = self.hearth_pending_wins.saturating_add(1);
            if self.carrying_mail && work.landmark == Building::Gatehouse {
                self.carrying_mail = false;
            }
        } else {
            self.recent_errors = (self.recent_errors + 1).min(9);
        }
        let verified_green = outcome.execution == ExecutionOutcome::Succeeded
            && outcome.verification == VerificationOutcome::Passed;
        let truthful_red = matches!(
            outcome.execution,
            ExecutionOutcome::Failed | ExecutionOutcome::Panicked
        ) || outcome.verification == VerificationOutcome::Failed;
        if verified_green {
            if self.outcome_err_streak >= 2 {
                self.clearing_until_beat = self.hearth.beats.saturating_add(life::CLEARING_BEATS);
            }
            self.outcome_err_streak = 0;
        } else if truthful_red {
            self.clearing_until_beat = 0;
            self.outcome_err_streak = self.outcome_err_streak.saturating_add(1).min(3);
        }
        let outcome_label = match outcome.execution {
            ExecutionOutcome::Succeeded => match outcome.verification {
                VerificationOutcome::Passed => "verified",
                VerificationOutcome::Failed => "verification failed",
                VerificationOutcome::Inconclusive => "verification inconclusive",
                VerificationOutcome::NotApplicable => "succeeded",
            },
            ExecutionOutcome::NotStarted => "not started",
            ExecutionOutcome::Failed => "failed",
            ExecutionOutcome::Denied => "denied",
            ExecutionOutcome::Cancelled => "cancelled",
            ExecutionOutcome::Panicked => "panicked",
        };
        self.activity = format!(
            "{} → {} · {outcome_label}",
            work.literal,
            work.landmark.label()
        );
        let verifier_credit = outcome.execution == ExecutionOutcome::Succeeded
            && outcome.verification == VerificationOutcome::Passed
            && !self.turn_verified_awarded;
        let research_credit = outcome.attributable_success()
            && work.activity == RealmActivity::Research
            && !self.turn_research_awarded;
        self.recent_work.push_back(work);
        while self.recent_work.len() > 16 {
            self.recent_work.pop_front();
        }
        if verifier_credit {
            self.turn_verified_awarded = true;
            self.verified_wins = self.verified_wins.saturating_add(1);
            self.credit_renown(format!("verification:{}", id.0), 4, "verification passed");
        }
        if research_credit {
            self.turn_research_awarded = true;
            self.credit_renown(format!("research:{}", id.0), 3, "research discovery");
        }
    }

    pub(crate) fn active_work(&self) -> impl Iterator<Item = &ActiveWork> {
        self.active_work.values()
    }

    pub(crate) fn latest_active_work(&self) -> Option<&ActiveWork> {
        self.active_work.values().max_by_key(|work| work.seq)
    }

    /// The wide-map knight's pose is a pure read of journey and work state.
    /// Only classified build/research work animates a settled worker; the
    /// activity-text fallback covers authored work such as the workshop.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn knight_pose(&self) -> KnightPose {
        if self.avatar != self.dest() || !self.avatar_vis_settled() {
            return KnightPose::Riding;
        }

        let active_here = self.active_build_landmark() == Some(self.target)
            || self.active_research_landmark() == Some(self.target);
        if self.avatar == self.building_pos(self.target)
            && (active_here || self.activity.contains("work"))
        {
            KnightPose::Working
        } else {
            KnightPose::Resting
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn knight_caption_verb(&self) -> &'static str {
        match self.knight_pose() {
            KnightPose::Riding => "riding to ",
            KnightPose::Working => "working at ",
            KnightPose::Resting if self.target == Building::Keep => "resting at ",
            KnightPose::Resting => "at ",
        }
    }

    pub(crate) fn event_diagnostics(&self) -> u64 {
        self.event_diagnostics
    }

    pub(crate) fn note_memory_health(&mut self, health: crate::memory_store::MemoryHealth) {
        self.memory_health = health;
    }

    pub(crate) fn note_atlas_status(
        &mut self,
        health: crate::atlas::AtlasHealth,
        review_count: usize,
    ) {
        self.atlas_health = health;
        self.atlas_review_count = review_count;
    }

    pub(crate) fn note_backplane_status(
        &mut self,
        clerk_health: crate::atlas_clerk::ClerkHealth,
        clerk_queue_depth: usize,
        resource_conflicts: usize,
    ) {
        self.clerk_health = clerk_health;
        self.clerk_queue_depth = clerk_queue_depth;
        self.resource_conflicts = resource_conflicts;
    }

    pub(crate) fn memory_health(&self) -> crate::memory_store::MemoryHealth {
        self.memory_health
    }

    pub(crate) fn causal_ribbon(&self) -> Option<String> {
        let work = self
            .active_work
            .values()
            .max_by_key(|work| work.seq)
            .or_else(|| self.recent_work.back())?;
        Some(Self::format_detailed_causal_ribbon(work, &work.literal))
    }

    /// Fit the causal ribbon by semantic priority, not by flat end clipping.
    /// Arguments disappear first, then outcome detail; the literal operation
    /// and its realm landmark remain visible for as long as the row permits.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn causal_ribbon_for_width(&self, width: usize) -> Option<String> {
        let work = self
            .active_work
            .values()
            .max_by_key(|work| work.seq)
            .or_else(|| self.recent_work.back())?;
        if width == 0 {
            return Some(String::new());
        }

        let full = Self::format_detailed_causal_ribbon(work, &work.literal);
        if cell_width(&full) <= width {
            return Some(full);
        }

        let without_arguments = Self::format_detailed_causal_ribbon(work, &work.operation);
        if cell_width(&without_arguments) <= width {
            return Some(without_arguments);
        }

        let destination = format!(" → {}", work.landmark.label());
        let relationship = format!("{}{destination}", work.operation);
        if cell_width(&relationship) <= width {
            return Some(relationship);
        }

        let destination_width = cell_width(&destination);
        if destination_width < width {
            let operation = fit_cells(&work.operation, width - destination_width);
            return Some(format!("{operation}{destination}"));
        }

        Some(fit_cells(&relationship, width))
    }

    fn format_detailed_causal_ribbon(work: &ActiveWork, literal: &str) -> String {
        let detail = match work.outcome {
            None => work.activity.running_label().to_string(),
            Some(outcome) => match outcome.execution {
                ExecutionOutcome::Succeeded => match outcome.verification {
                    VerificationOutcome::Passed => {
                        format!("{} · verified", work.summary)
                    }
                    VerificationOutcome::Failed => "verification failed".to_string(),
                    VerificationOutcome::Inconclusive => "verification inconclusive".to_string(),
                    VerificationOutcome::NotApplicable => {
                        if work.summary.is_empty() {
                            "succeeded".to_string()
                        } else {
                            work.summary.clone()
                        }
                    }
                },
                ExecutionOutcome::NotStarted => "not started".to_string(),
                ExecutionOutcome::Failed => "failed".to_string(),
                ExecutionOutcome::Denied => "denied".to_string(),
                ExecutionOutcome::Cancelled => "cancelled".to_string(),
                ExecutionOutcome::Panicked => "panicked".to_string(),
            },
        };
        format!("{literal} → {} · {detail}", work.landmark.label())
    }

    #[allow(dead_code)]
    pub(crate) fn realm_pulse(&self) -> Option<String> {
        let work = self
            .active_work
            .values()
            .max_by_key(|work| work.seq)
            .or_else(|| self.recent_work.back())?;
        Some(format!(
            "{} · {} · {}",
            work.landmark.label(),
            work.literal,
            Self::realm_pulse_status(work)
        ))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn realm_pulse_for_width(&self, width: usize) -> Option<String> {
        let work = self
            .active_work
            .values()
            .max_by_key(|work| work.seq)
            .or_else(|| self.recent_work.back())?;
        if width == 0 {
            return Some(String::new());
        }

        let prefix = format!("{} · ", work.landmark.label());
        let suffix = format!(" · {}", Self::realm_pulse_status(work));
        let fixed_width = cell_width(&prefix).saturating_add(cell_width(&suffix));
        if fixed_width < width {
            let operation = fit_cells(&work.pulse_operation, width - fixed_width);
            return Some(format!("{prefix}{operation}{suffix}"));
        }

        let compact = format!(
            "{} · {}",
            work.landmark.label(),
            Self::realm_pulse_status(work)
        );
        Some(fit_cells(&compact, width))
    }

    fn realm_pulse_status(work: &ActiveWork) -> &'static str {
        match work.outcome {
            None => "running",
            Some(outcome) if outcome.attributable_success() => {
                if outcome.verification == VerificationOutcome::Passed {
                    "verified"
                } else {
                    "succeeded"
                }
            }
            Some(ToolOutcome {
                execution: ExecutionOutcome::Denied,
                ..
            }) => "denied",
            Some(ToolOutcome {
                execution: ExecutionOutcome::Cancelled,
                ..
            }) => "cancelled",
            Some(ToolOutcome {
                execution: ExecutionOutcome::Panicked,
                ..
            }) => "panicked",
            Some(_) => "failed",
        }
    }

    #[cfg(test)]
    pub(crate) fn note_tool_call(&mut self, name: &str, args_summary: &str) {
        self.event_seq = self.event_seq.saturating_add(1);
        self.note_tool_call_event(
            ToolEventId(format!("world-test-{}", self.event_seq)),
            name,
            args_summary,
        );
    }

    // ─── the renown gate: one currency, four attributable award sources ─────

    fn credit_renown(&mut self, event_id: String, amount: u64, label: &str) {
        if amount == 0 || !self.credited_renown_ids.insert(event_id.clone()) {
            return;
        }
        self.credited_renown_order.push_back(event_id);
        while self.credited_renown_order.len() > 512 {
            if let Some(expired) = self.credited_renown_order.pop_front() {
                self.credited_renown_ids.remove(&expired);
            }
        }
        self.renown = self.renown.saturating_add(amount);
        self.hearth.prosperity = self.renown.min(999) as u32;
        self.sparkle_until = self.tick + 24;
        self.light_fireworks(24);
        self.flash_gain(format!("+{amount} renown · {label}"), 28);
        self.note_growth_announcements();
        self.save_rewards();
    }

    /// Flash a short gold reward readout at the end of the status line.
    fn flash_gain(&mut self, note: String, ticks: u64) {
        self.gain_note = note;
        self.gain_until = self.tick + ticks;
    }

    /// Light the celebration fireworks over the town for `ticks`. The default
    /// second burst rides the knight's target; a caller with a better stage
    /// (the forge on a promotion) sets `firework_focus` after.
    fn light_fireworks(&mut self, ticks: u64) {
        self.firework_from = self.tick;
        self.firework_until = self.tick + ticks;
        self.firework_focus = None;
    }

    /// Renown tier across the one shared construction ladder.
    pub(crate) fn level(&self) -> u32 {
        life::town_tier(self.renown.min(u64::from(u32::MAX)) as u32)
    }

    pub(crate) fn next_unlock(&self) -> Option<(u64, &'static str)> {
        [
            (10, "garden"),
            (24, "well"),
            (42, "lanterns"),
            (64, "banners"),
            (96, "docks"),
            (132, "windmill"),
            (172, "market"),
            (216, "keep towers"),
        ]
        .into_iter()
        .find(|(threshold, _)| self.renown < *threshold)
    }

    pub(crate) fn camera_mode_label(&self) -> &'static str {
        match self.camera.mode {
            CameraMode::Auto => "auto",
            CameraMode::Wide => "wide",
            CameraMode::Close => "close",
        }
    }

    pub(crate) fn weather_label(&self) -> &str {
        self.weather().trim().trim_start_matches('·').trim()
    }

    pub(crate) fn weather_report(&self) -> String {
        let clock = life::weather_for_beats(self.hearth.beats, self.seed);
        let resolved = self.outcome_weather();
        let sky = if resolved == life::Weather::Clearing {
            format!(
                "clearing-after-green, {} beats remaining",
                self.clearing_until_beat.saturating_sub(self.hearth.beats)
            )
        } else if self.outcome_err_streak >= 2 && resolved != clock {
            format!(
                "error streak {} forces {}",
                self.outcome_err_streak,
                resolved.label()
            )
        } else {
            format!(
                "error streak {} does not darken it",
                self.outcome_err_streak
            )
        };
        let banners = if self.completion_ceremony_active {
            "ceremony banners fly"
        } else {
            "ceremony banners rest"
        };
        format!(
            "Sky: {} — clock {}; {}; {}.",
            resolved.label(),
            clock.label(),
            sky,
            banners
        )
    }

    pub(crate) fn hearth_state(&self) -> &crate::hearth::HearthState {
        &self.hearth
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn construction_preview(&self) -> Option<(u64, &'static str)> {
        let (threshold, label) = self.next_unlock()?;
        self.renown
            .saturating_mul(5)
            .ge(&threshold.saturating_mul(3))
            .then_some((threshold, label))
    }

    fn reward_chrome(&self) -> String {
        if let Some((threshold, unlock)) = self.next_unlock() {
            format!(" Renown {}/{} · {unlock} next", self.renown, threshold)
        } else {
            format!(" Renown {} · realm complete", self.renown)
        }
    }

    /// Load v2 or migrate legacy progression in memory. Reading alone never
    /// rewrites the file; the next legitimate save performs the atomic v2 write.
    fn load_rewards(path: &std::path::Path) -> LoadedRewards {
        let Ok(text) = std::fs::read_to_string(path) else {
            return LoadedRewards::default();
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return LoadedRewards::default();
        };
        let hearth = serde_json::from_value::<crate::hearth::HearthState>(v["hearth"].clone())
            .unwrap_or_default();
        let xp = v["xp"].as_u64().unwrap_or(0);
        let coins = v["coins"].as_u64().unwrap_or(0);
        let renown = v["renown"].as_u64().unwrap_or_else(|| {
            u64::from(hearth.prosperity).max(xp.saturating_div(10).saturating_add(coins).min(999))
        });
        LoadedRewards {
            renown,
            verified_wins: v["verified_wins"].as_u64().unwrap_or(0),
            xp,
            coins,
            hearth,
        }
    }

    fn write_rewards_v2(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut hearth = self.hearth.clone();
        hearth.prosperity = self.renown.min(999) as u32;
        let body = serde_json::json!({
            "schema_version": 2,
            "renown": self.renown,
            "verified_wins": self.verified_wins,
            "hearth": hearth,
            "xp": self.xp,
            "coins": self.coins,
        })
        .to_string();
        let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&tmp, body)?;
        if let Err(error) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(error);
        }
        Ok(())
    }

    /// Persist v2 atomically. Streak and weather are session moods and die
    /// with the session; legacy XP/coin fields are retained for rollback.
    fn save_rewards(&self) {
        let Some(path) = &self.rewards_path else {
            return;
        };
        let _ = self.write_rewards_v2(path);
    }

    // ─── the village: the fleet mirrored into the town (village.rs) ─────────

    /// Wake the village: lay out the hamlet on open land (forge, granary, one
    /// cottage per fleet head) and adopt the persisted state. Layout snaps to
    /// the northwest district, clear of the landmarks, their keepers and the
    /// quintain — same margins the scenery keeps.
    pub(crate) fn enable_village(
        &mut self,
        state: crate::village::VillageState,
        path: Option<std::path::PathBuf>,
        head_ids: &[String],
    ) {
        let mut taken: Vec<(usize, usize)> = self.buildings.iter().map(|&(_, p)| p).collect();
        for &(kind, (bx, by)) in &self.buildings {
            if kind != Building::Keep {
                let kx = if bx + 2 < WORLD_W {
                    bx + 2
                } else {
                    bx.saturating_sub(2)
                };
                taken.push((kx, by + 1));
            }
        }
        taken.push(self.quintain);
        let (w, h) = (WORLD_W as f64, WORLD_H as f64);
        let forge_pos = self.nearest_land(w * 0.16, h * 0.22, &taken);
        taken.push(forge_pos);
        // The apprentice stands at the forge's porch — keep that tile open.
        let porch = if forge_pos.0 + 2 < WORLD_W {
            (forge_pos.0 + 2, forge_pos.1 + 1)
        } else {
            (forge_pos.0.saturating_sub(2), forge_pos.1 + 1)
        };
        taken.push(porch);
        let granary_pos = self.nearest_land(w * 0.26, h * 0.14, &taken);
        taken.push(granary_pos);
        taken.push((granary_pos.0 + 1, granary_pos.1)); // the fill gauge cell
        let mut cottages = Vec::with_capacity(head_ids.len());
        for (i, id) in head_ids.iter().enumerate() {
            let pos = self.nearest_land(w * (0.08 + 0.07 * i as f64), h * 0.34, &taken);
            taken.push(pos);
            cottages.push((id.clone(), pos));
        }
        // Replay the last cached chatter as the welcome-back line.
        let chatter_note = state.chatter.last().cloned().unwrap_or_default();
        let chatter_until = if chatter_note.is_empty() {
            0
        } else {
            self.tick + 90
        };
        self.village = Some(crate::village::Village {
            heads_up: head_ids.iter().map(|id| (id.clone(), false)).collect(),
            state,
            path,
            forge_pos,
            granary_pos,
            cottages,
            forge_up: false,
            training: false,
            new_samples: 0,
            gpu_free_mib: None,
            ollama_models: 0,
            eval_adapter: None,
            eval_base: None,
            last_gate_pass: None,
            soot_until: 0,
            chatter_note,
            chatter_until,
        });
    }

    /// Fold one sensing pulse into the village. Durable facts (adapter,
    /// dataset size, name, chatter) persist atomically; an adapter advancing
    /// past the saved one is THE promotion — fireworks over the forge, a
    /// permanent smithy upgrade, a celebration on the record. A failed eval
    /// gate is a soot puff (visible, gentle — never a storm). The forge not
    /// answering just darkens the buildings; state stays as saved.
    pub(crate) fn note_village(&mut self, pulse: crate::village::VillagePulse) {
        let Some(v) = &mut self.village else { return };
        let mut dirty = false;
        let mut promotion: Option<(String, u32)> = None;
        if let Some(name) = pulse.apprentice_name
            && v.state.apprentice_name.is_empty()
            && !name.is_empty()
        {
            v.state.apprentice_name = name;
            dirty = true;
        }
        if !pulse.heads.is_empty() {
            v.heads_up = pulse.heads.into_iter().map(|h| (h.id, h.up)).collect();
        }
        match pulse.forge {
            None => {
                v.forge_up = false;
                v.training = false;
            }
            Some(f) => {
                v.forge_up = true;
                v.training = f.training;
                v.new_samples = f.new_samples;
                v.gpu_free_mib = f.gpu_free_mib;
                v.ollama_models = f.ollama_models;
                v.eval_adapter = f.eval_loss_adapter;
                v.eval_base = f.eval_loss_base;
                if f.dataset_total != v.state.dataset_total {
                    v.state.dataset_total = f.dataset_total;
                    dirty = true;
                }
                // Soot on the falling edge only — one puff per failed gate.
                if f.gate_pass == Some(false) && v.last_gate_pass != Some(false) {
                    v.soot_until = self.tick + 36;
                }
                v.last_gate_pass = f.gate_pass;
                if let Some(adapter) = f.adapter
                    && v.state.last_adapter.as_deref() != Some(adapter.as_str())
                {
                    let old = v.apprentice_level();
                    let new = crate::village::adapter_level(&adapter);
                    v.state.last_adapter = Some(adapter.clone());
                    dirty = true;
                    if new > old {
                        // The permanent upgrade never rolls back.
                        v.state.smithy_level = v.state.smithy_level.max(new);
                        v.state
                            .celebrations
                            .push(format!("the forge promoted adapter {adapter}"));
                        while v.state.celebrations.len() > 20 {
                            v.state.celebrations.remove(0);
                        }
                        promotion = Some((adapter, new));
                    }
                }
            }
        }
        if let Some(line) = pulse.chatter {
            v.chatter_note = line.clone();
            v.chatter_until = self.tick + 90;
            v.state.chatter.push(line);
            while v.state.chatter.len() > 12 {
                v.state.chatter.remove(0);
            }
            dirty = true;
        }
        if dirty && let Some(p) = &v.path {
            crate::village::save_state(p, &v.state);
        }
        let forge_pos = v.forge_pos;
        let name = v.apprentice_display_name().to_string();
        if let Some((adapter, lvl)) = promotion {
            self.hearth.note_promotion();
            self.credit_renown(format!("promotion:{adapter}"), 10, "fleet promotion");
            self.firework_focus = Some(forge_pos);
            self.activity = format!("{name} promoted — adapter {adapter} (lv {lvl})");
        }
    }

    /// `/village` — the fleet, read off the town. Every line is a wired data
    /// source, so this doubles as the correspondence legend.
    pub(crate) fn village_report(&self) -> String {
        let Some(v) = &self.village else {
            return "/village: no village this session (ANGEL_VILLAGE=0 or no HOME) — \
                    the map is purely the knight's"
                .to_string();
        };
        let mut out = format!(
            "{}{}{}",
            crate::identity::VILLAGE_HEADING_PREFIX,
            self.town_name,
            crate::identity::VILLAGE_HEADING_SUFFIX
        );
        let forge_line = if !v.forge_up {
            "DARK (atlas unreachable — rendering from the saved state)"
        } else if v.training {
            "LIT — the hammer falls (training)"
        } else {
            "banked (idle, watching the inbox)"
        };
        out.push_str(&format!("  forge (head #2, ABT on atlas): {forge_line}\n"));
        let gate = match (v.last_gate_pass, v.eval_adapter, v.eval_base) {
            (Some(pass), Some(a), Some(b)) => {
                format!(
                    "{} ({a:.3} vs base {b:.3})",
                    if pass { "pass" } else { "fail" }
                )
            }
            (Some(pass), _, _) => if pass { "pass" } else { "fail" }.to_string(),
            _ => "—".to_string(),
        };
        out.push_str(&format!(
            "    adapter {} · apprentice {} lv{} · smithy lv{} · last gate: {gate}\n",
            v.state.last_adapter.as_deref().unwrap_or("none yet"),
            v.apprentice_display_name(),
            v.apprentice_level(),
            v.state.smithy_level,
        ));
        let gpu = v
            .gpu_free_mib
            .map(|m| format!(" · gpu free {m} MiB"))
            .unwrap_or_default();
        out.push_str(&format!(
            "    granary {} samples (+{} new) · fill {:.0}%{gpu} · ollama models {}\n",
            v.state.dataset_total,
            v.new_samples,
            f64::from(crate::village::granary_fill(v.state.dataset_total)) * 100.0,
            v.ollama_models,
        ));
        let cottages: Vec<String> = v
            .cottages
            .iter()
            .map(|(id, _)| format!("{id} {}", if v.head_up(id) { "lit" } else { "dark" }))
            .collect();
        if !cottages.is_empty() {
            out.push_str(&format!("  cottages: {}\n", cottages.join(" · ")));
        }
        out.push_str(&format!("  {}\n", self.reward_chrome().trim()));
        if let Some(c) = v.state.celebrations.last() {
            out.push_str(&format!(
                "  last celebration: {c} ({} on record)\n",
                v.state.celebrations.len()
            ));
        }
        if let Some(line) = v.state.chatter.last() {
            out.push_str(&format!(
                "  village chatter: \u{201C}{line}\u{201D} ({} cached)\n",
                v.state.chatter.len()
            ));
        }
        out
    }

    #[cfg(test)]
    pub(crate) fn note_tool_result(&mut self, name: &str, summary: &str) {
        let Some(id) = self
            .active_work
            .values()
            .filter(|work| work.literal.starts_with(name))
            .max_by_key(|work| work.seq)
            .map(|work| work.id.clone())
        else {
            self.event_diagnostics = self.event_diagnostics.saturating_add(1);
            return;
        };
        let lower = summary.to_ascii_lowercase();
        let failed = lower.starts_with("error")
            || lower.starts_with("failed")
            || lower.starts_with("tool error")
            || lower.contains('✗');
        self.note_tool_result_event(
            &id,
            name,
            summary,
            ToolOutcome {
                execution: if failed {
                    ExecutionOutcome::Failed
                } else {
                    ExecutionOutcome::Succeeded
                },
                verification: VerificationOutcome::NotApplicable,
            },
        );
    }

    /// Out-of-band notices: quota storms rain on the town; compaction is a
    /// chapel vigil.
    pub(crate) fn note_notice(&mut self, note: &str) {
        let lower = note.to_ascii_lowercase();
        if lower.contains("quota") || lower.contains("failing over") || lower.contains("benched") {
            self.storm_until = self.tick + 90;
        }
        if lower.contains("compact") {
            self.target = Building::Chapel;
            self.activity = "keeping vigil in the chapel".to_string();
        }
    }

    /// The loop launcher is an in-world workshop visit, not a background state
    /// transition. Move the knight to the smithy and sparkle the town while the
    /// modal is open.
    pub(crate) fn note_workshop(&mut self) {
        self.target = Building::Smithy;
        self.activity = "entering the tourney workshop".to_string();
        self.sparkle_until = self.tick + 30;
    }

    /// Sync the self-improvement loop into the town, once per frame. The
    /// quintain turns while `active`; a new `iteration` sparkles a finding;
    /// rising `agitated` (escalation / stall / awaiting SOTA) storms the town;
    /// and `done` gives a little parting sparkle when the loop finishes.
    ///
    /// Z1 thin adapter: besides the historical sparkle/storm/loop_* fields it
    /// also folds `Iteration`/`Stall` edges into the quest, so direct callers
    /// stay on the same adventure. The precise per-field events come from the
    /// `turn_io` `LoopMirror`, which drains before this runs.
    pub(crate) fn note_loop(
        &mut self,
        active: bool,
        iteration: usize,
        agitated: bool,
        done: bool,
        budget: Option<LoopBudgetSnapshot>,
    ) {
        let prev_iteration = self.loop_iteration;
        let prev_agitated = self.loop_agitated;
        if active {
            if iteration != self.loop_iteration {
                self.loop_iteration = iteration;
                self.sparkle_until = self.tick + 14;
            }
            if agitated && !self.loop_agitated {
                self.storm_until = self.tick + 90;
            }
            self.loop_agitated = agitated;
            self.loop_active = true;
            self.loop_budget = budget;
        } else {
            if self.loop_active && done {
                self.sparkle_until = self.tick + 30;
            }
            self.loop_active = false;
            self.loop_agitated = false;
            self.loop_iteration = 0;
            self.loop_budget = None;
        }
        if active && iteration != prev_iteration {
            self.quest
                .apply(AdventureEvent::Iteration { n: iteration }, self.tick);
        }
        if active && agitated != prev_agitated {
            // Agitation (escalation / stall / awaiting approval) is deep
            // danger; calming drops the fog entirely. The turn_io mirror
            // supplies the precise level when it runs first.
            let level = u8::from(agitated) * 3;
            self.quest.apply(AdventureEvent::Stall { level }, self.tick);
        }
    }

    /// Fold one harness fact into the quest (`adventure.rs`). Pure state, no
    /// rendering — Z2 draws it.
    pub(crate) fn note_adventure(&mut self, ev: AdventureEvent) {
        self.quest.apply(ev, self.tick);
    }

    /// Read-only quest view for the renderer/HUD (Z2/Z4).
    pub(crate) fn quest(&self) -> &Quest {
        &self.quest
    }

    /// The rolling tool classification `loop_kind_for` reads when a loop
    /// starts.
    pub(crate) fn tool_mix(&self) -> &ToolMix {
        &self.tool_mix
    }

    /// Where the avatar is currently walking: the quintain while a loop rests it
    /// between rounds, otherwise the landmark its work is happening in.
    fn dest(&self) -> (usize, usize) {
        if self.loop_active && self.target == Building::Keep {
            self.quintain
        } else {
            self.building_pos(self.target)
        }
    }

    /// The knight has settled at the quintain between rounds — the cue to expand
    /// the pane into the full loop visualization ("stepping up to the quintain").
    pub(crate) fn visiting_quintain(&self) -> bool {
        self.loop_active && self.target == Building::Keep && self.avatar == self.quintain
    }

    /// Test/diagnostic: whether [`note_loop`] last mirrored an active loop.
    #[cfg(test)]
    pub(crate) fn loop_mirrored_for_test(&self) -> bool {
        self.loop_active
    }

    /// A turn began: step out of the hut.
    pub(crate) fn turn_started(&mut self) {
        // active_work holds only the CURRENT turn's in-flight tool calls; an
        // entry is removed only by its matching ToolResult. A turn that ends
        // between ToolCall and ToolResult (truncation, cancel/hard-stop, worker
        // vanish) would otherwise orphan the entry forever — a slow leak plus an
        // O(n) per-frame scan over the documented multi-day loops. Wiping at the
        // turn boundary bounds it to one turn regardless of how the last ended.
        self.active_work.clear();
        self.note_adventure(AdventureEvent::TurnStarted);
        self.turn_epoch = self.turn_epoch.saturating_add(1);
        self.turn_verified_awarded = false;
        self.turn_research_awarded = false;
        if self.target == Building::Keep {
            self.activity = "riding out".to_string();
        }
    }

    /// The turn is over: wander home (errors leave the mail undelivered).
    /// Green turns pay out through the reward gate; gray turns just gut the
    /// streak and darken the weather — no payout is the whole punishment.
    pub(crate) fn turn_ended(&mut self, ok: bool) {
        self.turn_ended_ex(ok, /*present_stage*/ true);
    }

    /// A2: `ANGEL_BACKDROP=off` / Stage never painted — still clear agentviz and
    /// in-flight tool work so hidden state cannot grow, but skip renown disk
    /// I/O, fireworks, hearth beats, and other Stage presentation tax.
    pub(crate) fn turn_ended_hidden_stage(&mut self, ok: bool) {
        self.turn_ended_ex(ok, /*present_stage*/ false);
    }

    fn turn_ended_ex(&mut self, ok: bool, present_stage: bool) {
        self.carrying_mail = false;
        self.target = Building::Keep;
        self.note_adventure(AdventureEvent::TurnEnded { ok });
        if present_stage {
            self.activity = "riding back to the keep".to_string();
        }
        if ok {
            if present_stage {
                // Rainbow on recovery: the first green after a gray run.
                if self.fail_run >= 2 {
                    self.rainbow_until = self.tick + 90;
                }
                // A green turn dries the town out faster than tool results alone.
                self.recent_errors = self.recent_errors.saturating_sub(2);
                self.hearth.note_victory();
                self.streak = self.streak.saturating_add(1);
                // credit_renown → save_rewards (disk) + fireworks/flash.
                self.credit_renown(format!("turn:{}", self.turn_epoch), 1, "completed turn");
            }
            self.fail_run = 0;
        } else {
            if present_stage {
                self.recent_errors = (self.recent_errors + 2).min(9);
            }
            self.fail_run = self.fail_run.saturating_add(1);
            self.streak = 0;
        }
        // The campaign is over — disband whatever army was on the field.
        crate::agentviz::clear();
        self.muster.clear();
        self.muster_seq = 0;
        // Any tool call still in flight at turn end never got its ToolResult;
        // drop it so orphaned entries can't accumulate across turns.
        self.active_work.clear();
    }

    /// One animation frame. The knight walks a tile toward its target every
    /// other tick (integer game logic); its visual position and the camera ease
    /// smoothly on *every* tick.
    pub(crate) fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        // Banner / homecoming expiry follows the world clock even between
        // events (pure quest bookkeeping, no rendering).
        self.quest.observe_tick(self.tick);

        if self.tick.is_multiple_of(2) {
            let (tx, ty) = self.dest();
            let (ax, ay) = self.avatar;
            if (ax, ay) == (tx, ty) {
                if self.activity == "riding back to the keep" || self.activity == "riding out" {
                    self.activity = if self.loop_active && self.target == Building::Keep {
                        "tilting at the quintain".to_string()
                    } else {
                        self.target.arrive_activity().to_string()
                    };
                }
            } else {
                let route_is_current = self.route_goal == Some((tx, ty))
                    && self
                        .route
                        .front()
                        .is_some_and(|&(x, y)| x.abs_diff(ax) + y.abs_diff(ay) == 1);
                if !route_is_current {
                    self.route = self
                        .least_cost_route((ax, ay), (tx, ty))
                        .unwrap_or_default()
                        .into();
                    self.route_goal = Some((tx, ty));
                }

                if let Some((nx, ny)) = self.route.pop_front() {
                    if nx < ax {
                        self.facing_left = true;
                    } else if nx > ax {
                        self.facing_left = false;
                    }
                    self.avatar = (nx, ny);
                } else {
                    // Roads guarantee a route in generated worlds, but retain
                    // the old greedy step so malformed maps can never wedge.
                    let dx = tx as i32 - ax as i32;
                    let dy = ty as i32 - ay as i32;
                    if dx.abs() >= dy.abs() {
                        self.avatar.0 = (ax as i32 + dx.signum()) as usize;
                        self.facing_left = dx < 0;
                    } else {
                        self.avatar.1 = (ay as i32 + dy.signum()) as usize;
                    }
                }
            }
        }

        self.ease_avatar_vis();
        self.ease_travel_heading();
        self.tick_muster();
        self.tick_hearth();
        self.advance_camera();
    }

    /// The slow living loop, driven by tick count so the beat lands whether
    /// the cockpit idles at 200ms or runs 30fps — the world clock is world
    /// time, not wall time. Pure integer math, no allocation: the whole point
    /// is that the town lives *inside* the efficiency budget.
    fn tick_hearth(&mut self) {
        if !self.tick.is_multiple_of(crate::hearth::HEARTH_BEAT_TICKS) {
            return;
        }
        let wins = std::mem::take(&mut self.hearth_pending_wins);
        let (ollama, heads_lit, forge_hot, grain) = match &self.village {
            Some(v) => {
                let heads_lit = v.heads_up.iter().filter(|(_, up)| *up).count() as u32;
                let forge_hot = v.forge_up && v.training;
                let grain = (crate::village::granary_fill(v.state.dataset_total) * 100.0) as u32;
                (v.ollama_models as u32, heads_lit, forge_hot, grain)
            }
            None => (0, 0, false, 0),
        };
        self.hearth.beat(wins, ollama, heads_lit, forge_hot, grain);
        // Renown owns construction. Hearth telemetry may spend light and warm
        // the scene, but it cannot create a second unlock economy.
        self.hearth.prosperity = self.renown.min(999) as u32;
        // A prosperity crossing is a building raising: announce it once.
        self.note_growth_announcements();
        if self.hearth.due_to_save() {
            self.hearth.note_saved();
            self.save_rewards();
        }
    }

    /// The muster: a fan-out stage published through `agentviz` (swarm waves,
    /// judge panels, verify rounds, `spawn` formations) becomes an army on the
    /// field — one unit per seat, marching in through the gatehouse and forming
    /// up by stage kind. Skipped under test so the byte-identity fixtures never
    /// see another test's published stage.
    fn tick_muster(&mut self) {
        if cfg!(test) {
            return;
        }
        match crate::agentviz::current() {
            None => {
                self.muster.clear();
                self.muster_seq = 0;
            }
            Some(s) => {
                if s.seq != self.muster_seq {
                    self.muster_seq = s.seq;
                    self.form_muster(&s.name, s.agents.len().max(1));
                    self.apply_muster_states(&s.seat_states);
                }
            }
        }
        for u in &mut self.muster {
            let (tx, ty) = (u.slot.0 as f32, u.slot.1 as f32);
            u.vis.0 += (tx - u.vis.0) * 0.25;
            u.vis.1 += (ty - u.vis.1) * 0.25;
            if (u.vis.0 - tx).abs() < 0.05 && (u.vis.1 - ty).abs() < 0.05 {
                u.vis = (tx, ty);
            }
        }
    }

    /// Re-form the ranks for a new stage. Existing units keep their visual
    /// position (a wave→judge transition reads as the army re-forming, not
    /// re-spawning); new seats march in from the gatehouse.
    fn form_muster(&mut self, stage: &str, n: usize) {
        let n = n.min(16); // a horde larger than this stops reading as units
        let lower = stage.to_ascii_lowercase();
        let (glyph, ink) = if lower.contains("judge") {
            ('♛', DotColor::rgb(196, 181, 253))
        } else if lower.contains("verify") {
            ('♜', DotColor::rgb(96, 165, 250))
        } else if lower.contains("quorum") {
            ('♟', DotColor::rgb(45, 212, 191))
        } else if lower.contains("layer") || lower.contains("moa") {
            ('♞', DotColor::rgb(251, 146, 60))
        } else {
            // proposer waves, spawn panels, anything else: footsoldiers
            ('♟', DotColor::rgb(253, 224, 71))
        };
        // Muster field: open land south of the town centre, slots snapped to
        // land and kept off the landmarks so no one drills inside the smithy.
        let mut taken: Vec<(usize, usize)> = self.buildings.iter().map(|&(_, p)| p).collect();
        taken.push(self.quintain);
        let anchor = self.nearest_land(WORLD_W as f64 * 0.5, WORLD_H as f64 * 0.68, &taken);
        let gate = self.building_pos(Building::Gatehouse);
        let mut units = Vec::with_capacity(n);
        for i in 0..n {
            // Ranks of six, two cells apart, centred on the anchor.
            let (row, col) = (i / 6, i % 6);
            let row_len = (n - row * 6).min(6);
            let x = anchor.0 as i32 + col as i32 * 2 - (row_len as i32 - 1);
            let y = anchor.1 as i32 + row as i32 * 2 - 1;
            let slot = self.nearest_land(
                x.clamp(1, WORLD_W as i32 - 2) as f64,
                y.clamp(1, WORLD_H as i32 - 2) as f64,
                &taken,
            );
            taken.push(slot);
            // Keep a marching unit's position through a re-form; new seats
            // start at the gate.
            let vis = self
                .muster
                .get(i)
                .map(|u| u.vis)
                .unwrap_or((gate.0 as f32, gate.1 as f32));
            units.push(MusterUnit {
                vis,
                slot,
                glyph,
                ink,
            });
        }
        self.muster = units;
    }

    /// Per-seat status over a formed muster: a returned seat plants its
    /// banner, a failed seat dims red, a cut seat greys out. Ranks re-form off
    /// the same seq bump the update published, so this is a pure recolor over
    /// already-placed units — no pathing, no allocation, and a stage with no
    /// updates yet (empty states) is untouched.
    fn apply_muster_states(&mut self, states: &[crate::agentviz::SeatState]) {
        use crate::agentviz::SeatState;
        for (u, st) in self.muster.iter_mut().zip(states) {
            match st {
                SeatState::Running => {}
                SeatState::Returned => u.glyph = '⚑',
                SeatState::Failed => u.ink = DotColor::rgb(153, 27, 27),
                SeatState::Cut => u.ink = DotColor::rgb(87, 83, 78),
            }
        }
    }

    /// Glide the visual knight toward the integer `avatar` (render-only; all
    /// game logic keeps using `avatar`). Snaps when close so motion settles.
    fn ease_avatar_vis(&mut self) {
        let (tx, ty) = (self.avatar.0 as f32, self.avatar.1 as f32);
        self.avatar_vis.0 += (tx - self.avatar_vis.0) * 0.35;
        self.avatar_vis.1 += (ty - self.avatar_vis.1) * 0.35;
        if (self.avatar_vis.0 - tx).abs() < 0.05 && (self.avatar_vis.1 - ty).abs() < 0.05 {
            self.avatar_vis = (tx, ty);
        }
    }

    fn avatar_vis_settled(&self) -> bool {
        self.avatar_vis == (self.avatar.0 as f32, self.avatar.1 as f32)
    }

    /// Turn the first-person camera toward the direction of motion (the
    /// integer `avatar` leads the eased `avatar_vis`, so their delta is the
    /// current step direction). Shortest-arc easing keeps 90° Manhattan
    /// turns reading as one smooth swing; at rest the heading holds so the
    /// arrival view doesn't drift.
    fn ease_travel_heading(&mut self) {
        let dx = self.avatar.0 as f32 - self.avatar_vis.0;
        let dy = self.avatar.1 as f32 - self.avatar_vis.1;
        if dx.abs() < 0.05 && dy.abs() < 0.05 {
            return;
        }
        let desired = dy.atan2(dx);
        let mut turn = desired - self.travel_heading;
        while turn > std::f32::consts::PI {
            turn -= std::f32::consts::TAU;
        }
        while turn < -std::f32::consts::PI {
            turn += std::f32::consts::TAU;
        }
        self.travel_heading += turn * 0.3;
        // Keep the angle wrapped so quantized cache keys stay stable.
        if self.travel_heading.abs() > std::f32::consts::PI {
            self.travel_heading -= std::f32::consts::TAU * self.travel_heading.signum();
        }
    }

    /// The auto camera state machine + easing, once per tick. Deterministic:
    /// arrival plus a settle counter pick the altitude (a manual mode pins it),
    /// then zoom and centre ease toward their targets. The wide render never
    /// reads the camera, so this cannot perturb the travel view.
    fn advance_camera(&mut self) {
        let arrived = self.avatar == self.dest();
        if arrived {
            self.settle_ticks = self.settle_ticks.saturating_add(1);
            // Remember the place we actually reached: the next departure's
            // plate→world dissolve starts from this location's plate.
            self.plate_from = self.target;
            self.travel_ticks = 0;
        } else {
            // A new target (or still walking) pulls the camera straight out.
            self.interior = None;
            self.settle_ticks = 0;
            self.travel_ticks = self.travel_ticks.saturating_add(1);
        }
        let want_close = match self.camera.mode {
            CameraMode::Wide => false,
            CameraMode::Close => true,
            CameraMode::Auto => arrived && self.settle_ticks >= SETTLE_TICKS,
        };
        self.camera.zoom_target = if want_close { Z_CLOSE } else { Z_WIDE };
        // Close frames the knight together with the landmark it settled on; wide
        // just tracks the visual knight (the wide render ignores it anyway).
        let focus = if want_close {
            let (bx, by) = self.dest();
            (
                (self.avatar_vis.0 + bx as f32) / 2.0,
                (self.avatar_vis.1 + by as f32) / 2.0,
            )
        } else {
            self.avatar_vis
        };
        self.camera.center_target = focus;
        // Ease, then snap when close so the event loop can settle (invariant 5).
        self.camera.zoom += (self.camera.zoom_target - self.camera.zoom) * 0.25;
        if (self.camera.zoom - self.camera.zoom_target).abs() < 0.02 {
            self.camera.zoom = self.camera.zoom_target;
        }
        self.camera.center.0 += (focus.0 - self.camera.center.0) * 0.25;
        self.camera.center.1 += (focus.1 - self.camera.center.1) * 0.25;
        if (self.camera.center.0 - focus.0).abs() < 0.02
            && (self.camera.center.1 - focus.1).abs() < 0.02
        {
            self.camera.center = focus;
        }
    }

    /// `/world zoom`: cycle the manual camera override Auto → Wide → Close.
    /// Returns the new mode's label.
    pub(crate) fn cycle_camera_zoom(&mut self) -> &'static str {
        self.camera.cycle_mode()
    }

    /// Anything moving on screen? (Keeps the event loop on the fast tick — and
    /// must reach false once everything settles, or the loop spins forever.)
    pub(crate) fn animating(&self) -> bool {
        self.avatar != self.dest()
            || !self.route.is_empty()
            || !self.avatar_vis_settled()
            || self.camera.easing() || self.cinematic_zoom_pending()
            || self.tick < self.storm_until
            || self.tick < self.sparkle_until
            || self.tick < self.firework_until
            || self.tick < self.gain_until
            // Village beats are bounded windows; the forge's training glow is
            // deliberately NOT here — like the ambient layer it rides frames
            // other motion already asks for, so an idle world still parks.
            || self
                .village
                .as_ref()
                .is_some_and(|v| self.tick < v.soot_until || self.tick < v.chatter_until)
            || self.has_live_vignette()
            || !self.muster.is_empty()
    }

    /// A close-range vignette that animates on its own (the smithy hammer, the
    /// chapel candle, the Round Table's talkers) — keep the loop ticking so it
    /// stays alive. Gated on close zoom so the wide travel view still settles to
    /// idle (invariant 5): a knight studying in the scriptorium is not a
    /// vignette, so the loop parks.
    fn has_live_vignette(&self) -> bool {
        self.camera.zoom > 2.0
            && self.avatar == self.dest()
            && matches!(
                self.target,
                Building::Smithy | Building::Chapel | Building::RoundTable
            )
    }

    fn mood(&self) -> &'static str {
        if self.recent_errors >= 3 {
            "(x_x)"
        } else if self.tick < self.storm_until {
            "(o_o)"
        } else if self.carrying_mail || self.tick < self.sparkle_until {
            "(^o^)"
        } else {
            "(^-^)"
        }
    }

    /// Session weather, folded from real health: quota storms dominate, then
    /// the recovery rainbow, rain while errors pile up, clouds while the loop
    /// budget runs thin — the living weather's drizzle murmurs under all of
    /// them, and fair skies say nothing at all.
    fn weather(&self) -> &'static str {
        if self.tick < self.storm_until {
            " · storm"
        } else if self.tick < self.rainbow_until {
            " · rainbow"
        } else if self.raining() {
            " · rain"
        } else if self.budget_thin() {
            " · clouds"
        } else if self.outcome_weather() == life::Weather::Drizzle {
            " · drizzle"
        } else {
            ""
        }
    }

    /// Error rain: a gray run of turns or piled-up tool errors.
    fn raining(&self) -> bool {
        self.recent_errors >= 3 || self.fail_run >= 2
    }

    /// Under a quarter of the loop's token budget left → clouds gather.
    fn budget_thin(&self) -> bool {
        self.loop_budget.is_some_and(|b| {
            remaining_fraction(b.tokens_spent as u64, b.token_budget as u64) < 0.25
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn title(&self) -> String {
        let quest = if self.loop_active {
            format!(" · quest {}", self.loop_iteration)
        } else {
            String::new()
        };
        // The forge chip: training on Atlas reads off the pane title even when
        // the hamlet itself is out of frame.
        let forge = match &self.village {
            Some(v) if v.forge_up && v.training => " · forge lit",
            _ => "",
        };
        // A pinned camera override shows in the title; Auto adds nothing.
        let cam = match self.camera.mode {
            CameraMode::Close => " · close",
            CameraMode::Wide => " · wide",
            CameraMode::Auto => "",
        };
        let ward = if self.camera.zoom > Z_WIDE {
            self.district_at(self.camera.center)
                .map(|district| format!(" · {} Ward", district.name))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let activity = self.activity.chars().take(48).collect::<String>();
        // Chapel health follows the selected place, before clip-prone activity
        // and progression chrome, so the live Stage preserves it at standard
        // rail widths as well as full-body Realm focus.
        let memory = if self.target == Building::Chapel {
            match self.memory_health {
                crate::memory_store::MemoryHealth::Disabled => " · memory disabled",
                crate::memory_store::MemoryHealth::Healthy => " · memory healthy",
                crate::memory_store::MemoryHealth::Degraded => " · memory degraded",
            }
        } else {
            ""
        };
        let atlas = if self.target == Building::Chapel
            && self.atlas_health != crate::atlas::AtlasHealth::Disabled
        {
            format!(
                " · atlas {} · review {}",
                self.atlas_health.label(),
                self.atlas_review_count
            )
        } else {
            String::new()
        };
        // Off Castle Town the pane title names the place first: the adventure
        // is the headline, the town is only where it starts and ends.
        let region = match self.quest_region_chrome() {
            "" => String::new(),
            name => format!("{name} · "),
        };
        format!(
            " Realm · {}{}{} · {}{}{} · {} · {}{}{}{}{} ",
            region,
            self.town_name,
            ward,
            self.target.label(),
            memory,
            atlas,
            activity,
            self.reward_chrome().trim(),
            quest,
            forge,
            cam,
            self.weather(),
        )
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
impl World {
    fn render(&self, width: u16, height: u16) -> Text<'static> {
        let mut lines = Vec::new();
        if height > 1
            && let Some(image) = self.scryglass_frame_paced(
                width as usize,
                height.saturating_sub(1) as usize,
                false,
                0.0,
                0.0,
                1.05,
            )
        {
            for row in image.cells.chunks(width.max(1) as usize) {
                lines.push(Line::from(
                    row.iter()
                        .map(|cell| {
                            Span::styled(
                                cell.glyph.to_string(),
                                Style::new().fg(Color::Rgb(cell.fg[0], cell.fg[1], cell.fg[2])),
                            )
                        })
                        .collect::<Vec<_>>(),
                ));
            }
        }
        if height > 0 {
            lines.push(self.status_line(width as usize));
        }
        Text::from(lines)
    }
}
