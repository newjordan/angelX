//! First-person survey of the loop's connected Delve settlement. Production
//! World frames start on the PLAYER HALL rug and follow reachable passages to
//! miners, stockpiles, workshops and quarters. Only completed loop receipts
//! update the durable Floor and resource ledger in together_settlement.
//!
//! Rendering never excavates, builds, crafts or changes a playable checkpoint.
//! Measurement chests and a stalled torch are transient ceremonies; actual
//! record guardians keep their distinct fight. The generated crawl below stays
//! available for those guardian scenes and the existing deterministic fixtures.

pub(crate) mod dungeon;
mod fight;
pub(crate) mod party;
pub(crate) mod render;
pub(crate) mod settlement;
pub(crate) mod textures;

use dungeon::{Dungeon, Prop};
use render::{Eye, Kit, Tex, Thing, Torch};
use std::cell::RefCell;
use std::rc::Rc;

use super::adventure::{LoopKind, Region};
use super::overworld::arena;
use super::overworld::ink::Img;
use super::world3d::expedition::Work;
use crate::drive::together_shooter::{EnemyKind, Pack, RoomKind, bestiary, knights};

/// Whether the loop's 3D pane is the crawl (the default) or the older
/// expedition walk (`ANGEL_LOOP_VIEW=expedition`).
pub(crate) fn enabled() -> bool {
    std::env::var("ANGEL_LOOP_VIEW").map_or(true, |v| v.trim() != "expedition")
}

/// Everything a crawl frame is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Clock {
    pub(crate) region: Region,
    pub(crate) kind: LoopKind,
    pub(crate) iteration: usize,
    /// World ticks (40 a second) since this turn of the loop began, in
    /// steps of four; frozen far along when motion is off.
    pub(crate) elapsed: u32,
    pub(crate) work: Work,
    pub(crate) party: u8,
    pub(crate) danger: u8,
    /// Ticks into the fight with the guardian a settled submission brings,
    /// if one is on; whether it's a record (won) or a retreat; and how many
    /// guardians have risen this session.
    pub(crate) fight: Option<u32>,
    pub(crate) won: bool,
    pub(crate) bouts: u32,
    /// Ticks since a measurement came in, while it's news: the party finds a
    /// chest.
    pub(crate) found: Option<u32>,
}

impl Clock {
    pub(crate) fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        ("crawl", self).hash(&mut h);
        h.finish()
    }
}

/// Where motion is off, the clock stands far into a turn: arrived, still.
const STILL: u32 = 40 * 90;

impl super::World {
    #[cfg(test)]
    pub(crate) fn settlement_restore_for_test(
        &mut self,
        store: crate::drive::together_settlement::Store,
    ) {
        self.settlement = Some(store);
        self.settlement_scene.borrow_mut().take();
    }

    /// Existing Delve routes load the durable site even with no loop running.
    pub(crate) fn load_settlement(&mut self, player: &str) {
        if let Some(store) = &mut self.settlement {
            if let Err(error) = store.refresh() {
                self.settlement_error = Some(error);
                return;
            }
            self.settlement_error = None;
        } else {
            match crate::drive::together_settlement::Store::beside(
                self.rewards_path(),
                self.realm_seed(),
                player,
            ) {
                Ok(store) => {
                    self.settlement = Some(store);
                    self.settlement_error = None;
                }
                Err(error) => {
                    self.settlement_error = Some(error);
                    return;
                }
            }
        }
        self.settlement_scene.borrow_mut().take();
        self.ride_cache.borrow_mut().take();
    }

    pub(crate) fn sync_settlement(
        &mut self,
        st: &crate::drive::loop_ctl::LoopState,
        player: &str,
        tavern: bool,
    ) {
        if self.settlement.is_none() && self.settlement_error.is_none() {
            match crate::drive::together_settlement::Store::beside(
                self.rewards_path(),
                self.realm_seed(),
                player,
            ) {
                Ok(store) => self.settlement = Some(store),
                Err(error) => self.settlement_error = Some(error),
            }
        }
        if let Some(store) = &mut self.settlement {
            // A no-op sync is not evidence that a failed refresh recovered.
            // Revalidate disk state before clearing its error or admitting a
            // cached Floor, including when there is no active loop to sync.
            if self.settlement_error.is_some() {
                if let Err(error) = store.refresh() {
                    self.settlement_error = Some(error);
                    return;
                }
                self.settlement_scene.borrow_mut().take();
                self.ride_cache.borrow_mut().take();
            }
            let plans = st
                .durable_settlement_events()
                .map(|events| events.plans)
                .unwrap_or_default();
            match store.sync_with_plans(st, tavern, &plans) {
                Ok(changed) => {
                    self.settlement_error = None;
                    if changed {
                        self.settlement_scene.borrow_mut().take();
                        self.ride_cache.borrow_mut().take();
                    }
                }
                Err(error) => self.settlement_error = Some(error),
            }
        }
    }

    pub(crate) fn deposit_settlement_exhibits(
        &mut self,
        st: &crate::drive::loop_ctl::LoopState,
        workspace: &std::path::Path,
        name: Option<&str>,
    ) -> Result<usize, String> {
        if let Some(error) = &self.settlement_error {
            return Err(error.clone());
        }
        let count = self
            .settlement
            .as_mut()
            .ok_or("no saved settlement; sync a durable loop first")?
            .deposit(st, workspace, name)?;
        self.settlement_scene.borrow_mut().take();
        Ok(count)
    }

    pub(crate) fn settlement_site(
        &self,
    ) -> Result<Option<&crate::drive::together_settlement::Site>, &str> {
        if let Some(error) = &self.settlement_error {
            Err(error)
        } else {
            Ok(self.settlement.as_ref().map(|s| &s.site))
        }
    }

    pub(crate) fn crawl_frame(&self, clock: &Clock, size: (u32, u32)) -> image::RgbaImage {
        // Only an actual record guardian retains the separate fight ceremony.
        if clock.fight.is_some() {
            return frame(clock, size);
        }
        let mut cache = self.settlement_scene.borrow_mut();
        let scene = cache.get_or_insert_with(|| {
            let empty;
            let site = if let Some(store) = &self.settlement {
                &store.site
            } else {
                empty = crate::drive::together_settlement::Site::new(self.realm_seed(), "Host");
                &empty
            };
            settlement::Scene::of(site)
        });
        scene.events_frame(clock.elapsed, clock.danger, clock.found, size)
    }

    /// The crawl's clock now: the loop's state, how long into this turn of
    /// it (on the expedition's own walk clock), and what the tools are doing.
    pub(crate) fn crawl_clock(&self, motion: crate::ui::viz::lifecycle_viz::MotionMode) -> Clock {
        use super::world3d::expedition::FRAME_TICKS;
        let quest = self.quest();
        let elapsed = if motion == crate::ui::viz::lifecycle_viz::MotionMode::Full {
            let ticks = (quest.walk_half_ticks(self.tick) / 2).min(u128::from(u32::MAX)) as u32;
            ticks / FRAME_TICKS as u32 * FRAME_TICKS as u32
        } else {
            STILL
        };
        Clock {
            region: quest.region(),
            kind: quest.kind(),
            iteration: quest.iteration(),
            elapsed,
            work: self.expedition_playback(motion).work,
            party: quest.party(),
            danger: quest.danger().level(),
            fight: quest.record_fight(self.tick).map(|(t, _)| {
                if motion == crate::ui::viz::lifecycle_viz::MotionMode::Full {
                    t / FRAME_TICKS as u32 * FRAME_TICKS as u32
                } else {
                    // Still: mid-fight.
                    40 * 60
                }
            }),
            won: quest.record_fight(self.tick).is_none_or(|(_, won)| won),
            bouts: quest.bouts(),
            found: (motion == crate::ui::viz::lifecycle_viz::MotionMode::Full)
                .then(|| quest.fresh_measurement(self.tick))
                .flatten()
                .map(|t| t / FRAME_TICKS as u32 * FRAME_TICKS as u32),
        }
    }

    /// Whether a promoted record's guardian is being fought now.
    pub(crate) fn record_fight_on(&self) -> bool {
        enabled() && self.quest().record_fight(self.tick).is_some()
    }

    /// Whether the loop's pictures are the crawl now: a loop is out in a
    /// region (or a guardian is being fought), and no room is open.
    pub(crate) fn loop_crawl_on(&self) -> bool {
        enabled()
            && !self.ambient_interior_visible()
            && (super::world3d::region::stage_for(self.quest().region()).is_some()
                || self.record_fight_on())
    }
}

impl super::World {
    /// What the crawl's party is doing now, in two lines.
    pub(crate) fn crawl_log(
        &self,
        motion: crate::ui::viz::lifecycle_viz::MotionMode,
    ) -> [String; 2] {
        let clock = self.crawl_clock(motion);
        if clock.fight.is_some() {
            return log(&clock, self.quest().record_score());
        }
        if let Some(error) = &self.settlement_error {
            return [
                "Settlement unavailable · saved site left untouched".into(),
                error.clone(),
            ];
        }
        if clock.found.is_some() {
            return [
                "The surveyor prises open a chest of measurement receipts.".into(),
                "No excavation or crafted items are awarded by this ceremony.".into(),
            ];
        }
        if let Some(store) = &self.settlement {
            let site = &store.site;
            return [
                format!("{} · {}", site.player, site.activity()),
                format!(
                    "{} rooms built · {} stone · {} ore · {} tools · {} LOCAL RESEARCH stands · play /dungeon settlement; E inspects locally",
                    site.built,
                    site.resources.stone,
                    site.resources.ore,
                    site.resources.tools,
                    site.exhibits.len()
                ),
            ];
        }
        [
            "PLAYER HALL · waiting for completed loop receipts".into(),
            "No excavation is awarded by frame time.".into(),
        ]
    }
}

/// Where a region of the realm's loop lies in the Delve: its pack and how
/// deep.
fn delve_of(region: Region) -> (Pack, u32) {
    match region {
        Region::DarkForest => (Pack::Fungal, 5),
        Region::Swamp => (Pack::Archive, 4),
        Region::DragonKeep => (Pack::Hellforge, 3),
        Region::Homecoming => (Pack::Crypt, 1),
        Region::TheMines | Region::CastleTown => (Pack::Cavern, 2),
    }
}

/// One floor's plan: the dungeon, the route, and where the party stands in
/// each hall of it and which way it looks.
pub(crate) struct Plan {
    pub(crate) dungeon: Dungeon,
    pub(crate) route: Vec<usize>,
    pub(crate) spots: Vec<(i32, i32)>,
    pub(crate) looks: Vec<f32>,
    /// The way into each hall from the one before (none into the first).
    pub(crate) walks: Vec<Option<Rc<party::Walk>>>,
    pub(crate) depth: u32,
}

/// The plan for floor `index` of a loop in `region`, kept while it's in use.
pub(crate) fn plan(region: Region, kind: LoopKind, index: usize) -> Rc<Plan> {
    thread_local! {
        static LAST: RefCell<Option<((Region, LoopKind, usize), Rc<Plan>)>> = const { RefCell::new(None) };
    }
    LAST.with(|last| {
        if let Some((key, plan)) = last.borrow().as_ref()
            && *key == (region, kind, index)
        {
            return Rc::clone(plan);
        }
        let (pack, depth) = delve_of(region);
        let seed = 0x6372_6177_6c00_0000 ^ (index as u64) << 8 ^ (kind as u64) << 4 ^ region as u64;
        let floor = crate::drive::together_shooter::crawl_floor(depth, pack, seed);
        let dungeon = Dungeon::of(&floor);
        let route = party::route(&dungeon);
        let spots: Vec<(i32, i32)> = route.iter().map(|&h| party::spot(&dungeon, h)).collect();
        let looks: Vec<f32> = spots.iter().map(|&s| party::widest(&dungeon, s)).collect();
        let walks = (0..spots.len())
            .map(|k| {
                (k > 0)
                    .then(|| dungeon.path(spots[k - 1], spots[k]))
                    .flatten()
                    .map(|cells| Rc::new(party::Walk::along(&cells, looks[k - 1])))
            })
            .collect();
        let plan = Rc::new(Plan {
            dungeon,
            route,
            spots,
            looks,
            walks,
            depth,
        });
        *last.borrow_mut() = Some(((region, kind, index), Rc::clone(&plan)));
        plan
    })
}

/// Where the party is in a turn of the loop.
pub(crate) struct Pose {
    pub(crate) eye: Eye,
    /// Walking (and how far through its step), or arrived (and for how
    /// long).
    pub(crate) walking: Option<f32>,
    pub(crate) arrived: u32,
    /// The hall it's in or bound for, by route position.
    pub(crate) station: usize,
}

/// Floor and station for an iteration.
fn place(iteration: usize) -> (usize, usize) {
    (iteration / party::STATIONS, iteration % party::STATIONS)
}

pub(crate) fn pose(plan: &Plan, clock: &Clock) -> Pose {
    let (_, k) = place(clock.iteration);
    let k = k.min(plan.route.len().saturating_sub(1));
    let fov = 1.15;
    let look = plan.looks[k];
    let at = |(x, y): (f32, f32), heading: f32, bob: f32| Eye {
        x,
        y,
        heading,
        fov,
        z: 0.5 + bob,
        pitch: 0.0,
    };
    // The way here from the last hall (none at a floor's first hall).
    let walk = plan.walks[k].clone();
    let t = clock.elapsed;
    if let Some(walk) = &walk
        && let Some((p, heading, step)) = walk.at(t)
    {
        let bob = (step * std::f32::consts::PI).sin() * 0.03;
        return Pose {
            eye: at(p, heading, bob),
            walking: Some(step),
            arrived: 0,
            station: k,
        };
    }
    let ticks = walk.as_ref().map_or(0, |w| w.ticks());
    let since = t.saturating_sub(ticks);
    // Arrived: turn to look into the hall.
    let from = walk.as_ref().map_or(look, |w| w.last_heading(look));
    let k_turn = (since as f32 / (party::TURN * 2) as f32).min(1.0);
    let ease = k_turn * k_turn * (3.0 - 2.0 * k_turn);
    let by = {
        let d = (look - from).rem_euclid(std::f32::consts::TAU);
        if d > std::f32::consts::PI {
            d - std::f32::consts::TAU
        } else {
            d
        }
    };
    let mut eye = at(party::centre(plan.spots[k]), from + by * ease, 0.0);
    // What the work asks of the eye once it's there.
    match clock.work {
        Work::Dispatch => {
            // Scouting: a slow look from side to side into the dark.
            let s = (since as f32 / 90.0).sin();
            eye.heading += s * 0.75 * ease;
        }
        Work::Rest => eye.z = 0.38,
        Work::Study => eye.pitch = -0.06,
        _ => {}
    }
    // Breath.
    eye.z += ((t as f32) / 52.0).sin() * 0.008;
    Pose {
        eye,
        walking: None,
        arrived: since,
        station: k,
    }
}

/// The Delve's folk the party may meet in each region's halls.
/// The Delve's folk the party may meet in each region's halls, a different
/// one each turn of the loop.
fn folk(region: Region, iteration: usize) -> &'static str {
    let who: &[&str] = match region {
        Region::TheMines | Region::CastleTown => &["tobbin", "beaumains", "pip"],
        Region::DarkForest => &["maud", "merlin", "mabel"],
        Region::Swamp => &["merlin", "wren", "anselm"],
        Region::DragonKeep => &["kay", "ector", "dinadan"],
        Region::Homecoming => &["blaise", "wren", "tobbin"],
    };
    who[iteration % who.len()]
}

/// How long one monster lasts in a fight, and the swing's beat.
const FIGHT: u32 = 40 * 6;
const SWING: u32 = 30;

/// What the fight is up to: who's fighting, how far into it, whether a blow
/// just landed, and whether it has fallen.
fn fight(plan: &Plan, pose: &Pose) -> (Foe, u32, bool) {
    let hall = &plan.dungeon.halls[plan.route[pose.station]];
    let n = pose.arrived / FIGHT;
    let guardian = matches!(hall.kind, RoomKind::Stairs | RoomKind::Lair);
    let foe = if guardian && n.is_multiple_of(2) {
        Foe::Guardian
    } else if hall.roster.is_empty() {
        Foe::Monster(plain(plan.dungeon.pack, plan.depth))
    } else {
        Foe::Monster(hall.roster[n as usize % hall.roster.len()])
    };
    let into = pose.arrived % FIGHT;
    let struck = into > 40 && into % SWING < 5;
    (foe, into, struck)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Foe {
    Monster(EnemyKind),
    Guardian,
}

/// A floor's plainest monster, for a hall with nobody in it.
fn plain(pack: Pack, depth: u32) -> EnemyKind {
    pack.roster_at(depth)
        .first()
        .map_or(EnemyKind::Skeleton, |k| k.0)
}

/// The guardian a floor keeps: the first of its pack's.
fn guardian_art(pack: Pack) -> Option<Img> {
    static BOSSES: std::sync::OnceLock<Vec<crate::drive::together_shooter::bosses::Boss>> =
        std::sync::OnceLock::new();
    let boss = BOSSES
        .get_or_init(crate::drive::together_shooter::bosses::builtin)
        .iter()
        .find(|b| b.only_in == pack)?;
    Some(Img::from_rows(
        &boss.art.iter().map(String::as_str).collect::<Vec<_>>(),
    ))
}

/// The chorus's lines for one speaker, read once.
/// What a character says to a party that walks up: their greetings (lines
/// for `npc:<who>` or any `npc` cue), or anything of theirs if they have none.
fn lines_of(who: &str) -> Vec<String> {
    static SCRIPT: std::sync::OnceLock<Vec<crate::drive::together_chorus::Line>> =
        std::sync::OnceLock::new();
    let script = SCRIPT.get_or_init(crate::drive::together_chorus::script);
    let theirs = script.iter().filter(|l| l.who == who);
    let greetings: Vec<String> = theirs
        .clone()
        .filter(|l| {
            l.cue.starts_with("npc") || l.cue.starts_with("merlin") || l.cue.starts_with("bar")
        })
        .map(|l| l.words.clone())
        .collect();
    if greetings.is_empty() {
        theirs.map(|l| l.words.clone()).collect()
    } else {
        greetings
    }
}

/// A frame of the crawl, `w` by `h`.
pub(crate) fn frame(clock: &Clock, (w, h): (u32, u32)) -> image::RgbaImage {
    let (index, _) = place(clock.iteration);
    let plan = plan(clock.region, clock.kind, index);
    if let Some(t) = clock.fight {
        return fight::frame(&plan, clock, t, (w, h));
    }
    let pose = pose(&plan, clock);
    let faces = textures::faces();
    let decals = wall_decals(clock.elapsed);
    let kit = Kit {
        walls: &faces.walls,
        decals: &decals,
        floor: &faces.floor,
        water: &faces.water,
        lava: &faces.lava,
        stairs: &faces.stairs,
        ceiling: &faces.ceiling,
    };
    let t = clock.elapsed;
    // A stalled loop: the torch gutters, and the dark comes closer.
    let unsteady = 1.0 + f32::from(clock.danger) * 1.5;
    let mut torch = Torch {
        reach: 5.5 - 0.9 * f32::from(clock.danger.min(3)),
        gain: 1.0
            + ((t as f32) * 0.37).sin() * 0.05 * unsteady
            + ((t as f32) * 1.13).sin() * 0.03 * unsteady,
        glow: None,
        sconces: sconces_near(&plan, (pose.eye.x, pose.eye.y)),
    };
    // Pictures for this frame: kept alive while the things borrow them.
    let mut art: Vec<Tex> = Vec::new();
    let place_thing = |art: &mut Vec<Tex>, img: Img| {
        art.push(Tex::from_img(&img));
        art.len() - 1
    };
    let mut specs: Vec<(f32, f32, usize, f32, f32, bool, bool)> = Vec::new();
    let tick = t / 4;
    // The floor's props near the party.
    for &(x, y, prop) in &plan.dungeon.props {
        if (x - pose.eye.x).hypot(y - pose.eye.y) > 10.0 {
            continue;
        }
        let (img, tall, lit) = match prop {
            Prop::Tomb => (arena::crawl_prop("tomb", tick), 0.55, false),
            Prop::Boulder => {
                art.push(textures::sprites().boulder.clone());
                specs.push((x, y, art.len() - 1, 0.42, 0.0, false, false));
                continue;
            }
            Prop::Mushroom => (arena::crawl_prop("mushroom", tick), 0.8, false),
            Prop::EyePillar => (arena::crawl_prop("eye_pillar", tick), 1.0, false),
            Prop::Crate | Prop::Workbench | Prop::Bed | Prop::Exhibit => {
                let (tex, height) = settlement::furnishing(prop);
                let i = art.len();
                art.push(tex.clone());
                specs.push((x, y, i, height, 0.0, false, false));
                continue;
            }
        };
        let i = place_thing(&mut art, img);
        specs.push((x, y, i, tall, 0.0, lit, false));
    }
    // A treasure hall's chest, by the wall to one side.
    if pose.walking.is_none()
        && plan.dungeon.halls[plan.route[pose.station]].kind == RoomKind::Treasure
    {
        let look = plan.looks[pose.station];
        let (cx, cy) = party::centre(plan.spots[pose.station]);
        let d = (party::open_ahead(&plan.dungeon, plan.spots[pose.station], look) as f32).min(3.0);
        let (x, y) = (
            cx + look.cos() * d - look.sin() * 1.2,
            cy + look.sin() * d + look.cos() * 1.2,
        );
        art.push(textures::sprites().chest.clone());
        let i = art.len() - 1;
        specs.push((x, y, i, 0.42, 0.0, false, false));
    }
    // What stands before the party, by the work.
    let ahead = |dist: f32| {
        let at = plan.spots[pose.station];
        let room = party::open_ahead(&plan.dungeon, at, plan.looks[pose.station]) as f32;
        let d = dist.min(room + 0.4).max(1.2);
        let (cx, cy) = party::centre(at);
        let look = plan.looks[pose.station];
        (cx + look.cos() * d, cy + look.sin() * d)
    };
    if let Some(found) = clock.found.filter(|_| pose.walking.is_none()) {
        // A measurement came in: a chest, prised open, gold in its light.
        let (x, y) = ahead(1.7);
        let sprites = textures::sprites();
        art.push(if found > 40 {
            sprites.chest_open.clone()
        } else {
            sprites.chest.clone()
        });
        specs.push((x, y, art.len() - 1, 0.4, 0.0, false, false));
        if found > 40 {
            let coin = Tex::from_img(&Img::from_rows(&fight::COIN));
            let look = plan.looks[pose.station];
            for k in 0..4u32 {
                let rise = (((found - 40) as f32 / 30.0 + k as f32 * 0.7) % 2.0) * 0.18;
                let side = (k as f32 - 1.5) * 0.18;
                art.push(coin.clone());
                specs.push((
                    x - look.cos() * 0.1 - look.sin() * side,
                    y - look.sin() * 0.1 + look.cos() * side,
                    art.len() - 1,
                    0.12,
                    0.36 + rise,
                    true,
                    false,
                ));
            }
            torch.glow = Some((x - look.cos() * 0.4, y - look.sin() * 0.4, 1.4));
        }
    } else if pose.walking.is_none() {
        match clock.work {
            Work::Forge => {
                let (foe, into, struck) = fight(&plan, &pose);
                // In from the dark, then toe to toe; at the end it falls.
                let come = (into as f32 / 40.0).min(1.0);
                let (x, y) = ahead(4.2 - 1.9 * come);
                let falling = into > FIGHT - 30;
                let (img, tall) = match foe {
                    Foe::Monster(kind) => (arena::crawl_enemy(kind, tick), 0.7),
                    Foe::Guardian => (
                        guardian_art(plan.dungeon.pack)
                            .unwrap_or_else(|| arena::crawl_enemy(EnemyKind::Demon, tick)),
                        1.3,
                    ),
                };
                let img = if struck { flash(&img) } else { img };
                let sink = if falling {
                    (into - (FIGHT - 30)) as f32 / 30.0
                } else {
                    0.0
                };
                let i = place_thing(&mut art, img);
                specs.push((x, y, i, tall * (1.0 - sink * 0.9), 0.0, false, false));
            }
            Work::Study => {
                let (x, y) = ahead(1.6);
                art.push(textures::sprites().lectern.clone());
                let i = art.len() - 1;
                specs.push((x, y, i, 0.62, 0.0, false, false));
                // A candle on the lectern.
                torch.glow = Some((x, y, 1.4));
            }
            Work::Council => {
                let (x, y) = ahead(2.2);
                if let Some(img) = arena::crawl_figure(folk(clock.region, clock.iteration)) {
                    let i = place_thing(&mut art, img);
                    specs.push((x, y, i, 0.72, 0.0, false, false));
                }
            }
            Work::Rest => {
                let (x, y) = ahead(1.8);
                art.push(textures::sprites().campfire[(tick / 2 % 2) as usize].clone());
                let i = art.len() - 1;
                specs.push((x, y, i, 0.5, 0.0, true, false));
                // The fire lights the hall round it.
                torch.glow = Some((x, y, 3.2));
            }
            Work::Dispatch => {
                // Something in the dark, watching back.
                let hall = &plan.dungeon.halls[plan.route[pose.station]];
                if let Some(&kind) = hall.roster.first() {
                    let (x, y) = ahead(5.5);
                    let i = place_thing(&mut art, arena::crawl_enemy(kind, tick));
                    specs.push((x, y, i, 0.65, 0.0, false, false));
                }
            }
        }
    }
    let things: Vec<Thing> = specs
        .iter()
        .map(|&(x, y, i, height, lift, lit, flip)| Thing {
            x,
            y,
            tex: &art[i],
            height,
            lift,
            lit,
            flip,
        })
        .collect();
    let mut out = render::render(&plan.dungeon.grid, &pose.eye, &things, &kit, torch, (w, h));
    hand(&mut out, clock, &pose);
    // Down a stair to a new floor: the dark lifts as the eyes adjust.
    if let Some(k) = descending(clock) {
        for p in out.pixels_mut() {
            for c in 0..3 {
                p[c] = (p[c] as f32 * k) as u8;
            }
        }
    }
    out
}

/// Ticks for the eyes to adjust after a stair.
const ADJUST: u32 = 60;

/// How far through adjusting to the dark of a new floor the party is, if
/// it has just come down a stair.
fn descending(clock: &Clock) -> Option<f32> {
    let (index, k) = place(clock.iteration);
    (index > 0 && k == 0 && clock.elapsed < ADJUST).then(|| clock.elapsed as f32 / ADJUST as f32)
}

/// What hangs on the walls this moment: a sconce (in its flicker), a
/// banner; in the order the dungeon names them.
fn wall_decals(t: u32) -> [&'static Tex; 2] {
    let art = textures::sprites();
    [&art.sconce[(t / 6 % 2) as usize], &art.banner]
}

/// The three wall sconces nearest `(x, y)`, within reach of being seen.
fn sconces_near(plan: &Plan, (x, y): (f32, f32)) -> [Option<(f32, f32)>; 3] {
    dungeon_sconces_near(&plan.dungeon, (x, y))
}

fn dungeon_sconces_near(dungeon: &Dungeon, (x, y): (f32, f32)) -> [Option<(f32, f32)>; 3] {
    let mut near: Vec<(f32, (f32, f32))> = dungeon
        .sconces
        .iter()
        .map(|&(sx, sy)| ((sx - x).hypot(sy - y), (sx, sy)))
        .filter(|(d, _)| *d < 8.0)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = [None; 3];
    for (slot, (_, at)) in out.iter_mut().zip(near) {
        *slot = Some(at);
    }
    out
}

/// A picture washed white for the instant a blow lands.
fn flash(img: &Img) -> Img {
    let mut out = img.clone();
    for y in 0..img.h {
        for x in 0..img.w {
            if let Some(c) = img.get(x, y) {
                out.set(x, y, [c[0] / 2 + 120, c[1] / 2 + 120, c[2] / 2 + 120]);
            }
        }
    }
    out
}

fn hand(out: &mut image::RgbaImage, clock: &Clock, pose: &Pose) {
    let (mut frame, dx, mut dy) = (0, 0.0, 0.0);
    if let Some(step) = pose.walking {
        dy += (step * std::f32::consts::TAU).sin().abs() * 2.0;
    }
    if pose.walking.is_none() && clock.work == Work::Forge && pose.arrived % FIGHT > 40 {
        // Drawn back as the beat comes round, across as it lands.
        frame = match pose.arrived % SWING {
            p if p >= SWING - 10 => 1,
            p if p < 8 => 2,
            _ => 0,
        };
    }
    hands(out, (frame, (dx, dy)), clock.elapsed);
}

/// The knight's hands at the bottom of the view, as the old games drew
/// them: the torch on the left, flickering; the sword on the right, in one
/// of its three frames (at rest, drawn back, mid-swing) and moved by
/// `(dx, dy)` of its own pixels.
fn hands(out: &mut image::RgbaImage, (frame, (dx, dy)): (usize, (f32, f32)), t: u32) {
    let art = textures::sprites();
    let h = out.height() as f32;
    let w = out.width() as f32;
    // Drawn for a view 160 tall; the cuffs run off the bottom edge.
    let s = h / 160.0;
    let torch = &art.torch[(t / 6 % 2) as usize];
    stamp(out, torch, (0.0, h - torch.h as f32 * s + 2.0 * s), s);
    let sword = &art.sword[frame.min(2)];
    stamp(
        out,
        sword,
        (
            w - sword.w as f32 * s + dx * s,
            h - sword.h as f32 * s + dy * s,
        ),
        s,
    );
}

/// A picture laid over the view at `(x, y)`, `s` view pixels to its pixel.
fn stamp(out: &mut image::RgbaImage, tex: &Tex, (x, y): (f32, f32), s: f32) {
    let (w, h) = (out.width() as i32, out.height() as i32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (x1, y1) = (
        (x + tex.w as f32 * s).ceil() as i32,
        (y + tex.h as f32 * s).ceil() as i32,
    );
    for py in y0.max(0)..y1.min(h) {
        for px in x0.max(0)..x1.min(w) {
            let u = (px as f32 + 0.5 - x) / (tex.w as f32 * s);
            let v = (py as f32 + 0.5 - y) / (tex.h as f32 * s);
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            if let Some(c) = tex.at(u, v) {
                out.put_pixel(px as u32, py as u32, image::Rgba([c[0], c[1], c[2], 255]));
            }
        }
    }
}

/// Who walks, in a sentence: "The party", or the one knight by name when
/// the formation is a single knight.
fn company(names: &[&str]) -> String {
    if names.len() == 1 {
        names[0].to_string()
    } else {
        "The party".to_string()
    }
}

/// The party's names, as many as walk: each seat's knight from the house
/// serving it, or the Keep's own household when no house serves.
fn party_names(n: u8) -> Vec<&'static str> {
    let serving = crate::stage::houses::serving();
    knights::COMPANY
        .iter()
        .take(usize::from(n.clamp(1, 4)))
        .enumerate()
        .map(|(i, k)| {
            let house = crate::stage::houses::for_seat(&serving, i as u32 + 1);
            k.of_house(house).map_or(k.name, |h| h.name.as_str())
        })
        .collect()
}

fn monster_name(kind: EnemyKind) -> String {
    bestiary::ENTRIES
        .iter()
        .find(|e| e.kind == kind)
        .map_or_else(|| format!("{kind:?}"), |e| e.name.to_string())
        .to_lowercase()
}

/// What's happening, in two lines for under the picture.
pub(crate) fn log(clock: &Clock, score: Option<&str>) -> [String; 2] {
    let (index, _) = place(clock.iteration);
    let plan = plan(clock.region, clock.kind, index);
    if let Some(t) = clock.fight {
        return fight::log(clock, &plan, t, score);
    }
    let pose = pose(&plan, clock);
    let names = party_names(clock.party);
    let place_name = plan.dungeon.pack.name();
    let floor = index + 1;
    if descending(clock).is_some() {
        return [
            format!("Down the stair to floor {floor} of {place_name}."),
            "Eyes adjust to the dark.".to_string(),
        ];
    }
    if let Some(found) = clock.found.filter(|_| pose.walking.is_none()) {
        return [
            format!("{} prises open a chest.", names[0]),
            if found > 40 {
                "A measurement is in. Gold glints in the torchlight.".to_string()
            } else {
                "A measurement is in.".to_string()
            },
        ];
    }
    let hall = &plan.dungeon.halls[plan.route[pose.station]];
    let mut said = log_at(clock, &plan, &pose, &names, place_name, floor);
    if pose.walking.is_none() && hall.kind == RoomKind::Treasure && clock.work != Work::Forge {
        said[1] = "An iron-bound chest waits by the wall.".to_string();
    }
    // A stalled loop, said plainly where nobody else is talking.
    if clock.danger >= 2
        && (pose.walking.is_some() || matches!(clock.work, Work::Rest | Work::Dispatch))
    {
        said[1] = "The torch gutters; the dark presses in.".to_string();
    }
    said
}

fn log_at(
    clock: &Clock,
    plan: &Plan,
    pose: &Pose,
    names: &[&'static str],
    place_name: &str,
    floor: usize,
) -> [String; 2] {
    let lead = names[0];
    if pose.walking.is_some() {
        let next = &plan.dungeon.halls[plan.route[pose.station]];
        let bound = if matches!(next.kind, RoomKind::Stairs | RoomKind::Lair) {
            "for the stairs down"
        } else {
            "deeper in"
        };
        return [
            format!("{lead} leads the way through {place_name}, {bound}."),
            format!("Floor {floor} of the crawl. Somewhere ahead, water drips."),
        ];
    }
    match clock.work {
        Work::Forge => {
            let (foe, into, struck) = fight(plan, pose);
            let name = match foe {
                Foe::Monster(kind) => format!("the {}", monster_name(kind)),
                Foe::Guardian => "the guardian".to_string(),
            };
            let striker = names[(pose.arrived / SWING) as usize % names.len()];
            let first = if into > FIGHT - 30 {
                format!("{name} falls. {striker} wipes the blade.")
            } else if struck {
                format!("{striker} strikes {name}.")
            } else {
                format!("{lead} squares up to {name}.")
            };
            [first, format!("In {place_name}, floor {floor}.")]
        }
        Work::Study => {
            let hall = &plan.dungeon.halls[plan.route[pose.station]];
            let kind = hall
                .roster
                .get((pose.arrived / 200) as usize % hall.roster.len().max(1))
                .copied()
                .unwrap_or_else(|| plain(plan.dungeon.pack, plan.depth));
            let says = bestiary::ENTRIES
                .iter()
                .find(|e| e.kind == kind)
                .map_or("", |e| e.says);
            [
                format!(
                    "{lead} reads at a lectern, a tome open on the {}.",
                    monster_name(kind)
                ),
                says.to_string(),
            ]
        }
        Work::Council => {
            let who = folk(clock.region, clock.iteration);
            let name = crate::drive::together_chorus::name(who).to_string();
            let said = lines_of(who);
            let line = said
                .get((pose.arrived / 240) as usize % said.len().max(1))
                .map_or_else(String::new, |words| format!("{name}: {words}"));
            let with = if names.len() == 1 {
                names[0].to_string()
            } else {
                "the party".to_string()
            };
            [format!("{name} has a word with {with}."), line]
        }
        Work::Dispatch => [
            format!("{lead} scouts ahead, the torch held high."),
            format!("Something in {place_name} watches back."),
        ],
        Work::Rest => [
            format!("{} rests by a fire in {place_name}.", company(names)),
            format!("{} keeps the watch.", names[names.len() - 1]),
        ],
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/world_viz/crawl__tests.rs"]
mod tests;
