//! The live map: the overworld scene read from the running [`World`], and
//! the knight's walk along the realm's roads.
//!
//! The pixel realm has its own geography, so the knight keeps his own place
//! on it. When the world's destination changes he takes the cheapest route
//! to the new place's stand tile — roads are quick, open ground slower,
//! swamp slowest; trees, rock, water and every structure are closed.
//! Everything else in the scene is a direct read of world state: nothing
//! lights up that the harness did not do.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::OnceLock;

use super::super::adventure::Region;
use super::super::{Building, RealmActivity, World};
use super::glass::{GLASS_H, GLASS_W, Glass, picture_from_rgba};
use super::ink::Img;
use super::ink::{BANK, PALETTE, SIGNAL_BANK, rgb};
use super::kit::Tool;
use super::light::DUSK;
use super::map::{MAP_H, MAP_W, Place, Realm, STRUCTURES, TILE};
use super::scene::{Joust, Knight, Scene, Soldier, SoldierState, Ward, Weather};

/// Walking pace in world pixels per world tick (the world ticks at 40 Hz).
const PACE: f32 = 1.5;
/// The camera's dead zone, half-width and half-height in map pixels, and
/// how far above his feet the camera looks (his middle, not his boots).
const DEAD_X: f32 = 40.0;
const DEAD_Y: f32 = 28.0;
const FOCUS_LIFT: f32 = 8.0;

/// Move the camera just enough to keep `focus` inside the dead zone.
pub(crate) fn follow(cam: (f32, f32), focus: (f32, f32)) -> (f32, f32) {
    let axis = |c: f32, f: f32, dead: f32| {
        if f - c > dead {
            f - dead
        } else if c - f > dead {
            f + dead
        } else {
            c
        }
    };
    (axis(cam.0, focus.0, DEAD_X), axis(cam.1, focus.1, DEAD_Y))
}

/// Trail samples kept, and the samples between companions (about 13 px).
const TRAIL: usize = 64;
const FOLLOW_GAP: usize = 9;

/// The knight's walk across the realm.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Walker {
    x: f32,
    y: f32,
    goal: Place,
    path: VecDeque<(i32, i32)>,
    /// Where he has just been, newest first — the party walks in his steps.
    trail: VecDeque<(f32, f32)>,
    /// Where the map camera looks. It holds still while the knight moves
    /// inside a dead zone and follows him out of it; it never zooms.
    cam: (f32, f32),
}

impl Default for Walker {
    fn default() -> Walker {
        let home = Knight::at_place(Place::Keep);
        Walker {
            x: home.x,
            y: home.y,
            goal: Place::Keep,
            path: VecDeque::new(),
            trail: VecDeque::new(),
            cam: (home.x, home.y - FOCUS_LIFT),
        }
    }
}

fn feet((tx, ty): (i32, i32)) -> (f32, f32) {
    ((tx * TILE + TILE / 2) as f32, ((ty + 1) * TILE - 2) as f32)
}

impl Walker {
    /// One world tick toward `goal`; a new goal reroutes from where he stands.
    pub(crate) fn toward(&mut self, goal: Place) {
        if goal != self.goal {
            self.goal = goal;
            self.path = route(self.tile(), goal.stand()).into();
        }
        let before = (self.x, self.y);
        let mut stride = PACE;
        while stride > 0.0 {
            let Some(&next) = self.path.front() else {
                break;
            };
            let (gx, gy) = feet(next);
            let (dx, dy) = (gx - self.x, gy - self.y);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= stride {
                (self.x, self.y) = (gx, gy);
                stride -= d;
                self.path.pop_front();
            } else {
                self.x += dx / d * stride;
                self.y += dy / d * stride;
                stride = 0.0;
            }
        }
        if (self.x, self.y) != before {
            self.trail.push_front(before);
            self.trail.truncate(TRAIL);
        }
        self.cam = follow(self.cam, (self.x, self.y - FOCUS_LIFT));
    }

    /// The point the map camera centres on.
    pub(crate) fn camera(&self) -> (f32, f32) {
        self.cam
    }

    /// Where `n` companions stand: at intervals along his trail, or in a
    /// short file behind him before he has walked anywhere.
    pub(crate) fn followers(&self, n: usize) -> Vec<(f32, f32)> {
        (1..=n)
            .map(|i| match self.trail.get(i * FOLLOW_GAP) {
                Some(&p) => p,
                None => self
                    .trail
                    .back()
                    .copied()
                    .map_or((self.x - 12.0 * i as f32, self.y), |(x, y)| {
                        (x - 3.0 * i as f32, y)
                    }),
            })
            .collect()
    }

    pub(crate) fn knight(&self) -> Knight {
        Knight {
            x: self.x,
            y: self.y,
            walking: !self.path.is_empty(),
        }
    }

    fn tile(&self) -> (i32, i32) {
        (
            (self.x / TILE as f32).floor() as i32,
            (self.y / TILE as f32).floor() as i32,
        )
    }
}

fn closed_tiles() -> &'static [bool] {
    static CLOSED: OnceLock<Vec<bool>> = OnceLock::new();
    CLOSED.get_or_init(|| {
        let realm = Realm::get();
        let mut closed = vec![false; (MAP_W * MAP_H) as usize];
        for (x0, y0, w, h) in STRUCTURES {
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    if !matches!(realm.at(x, y), b'=' | b':' | b'H') {
                        closed[(y * MAP_W + x) as usize] = true;
                    }
                }
            }
        }
        closed
    })
}

/// What a step onto `(x, y)` costs, or `None` where no one walks.
fn step_cost(x: i32, y: i32) -> Option<u32> {
    if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H || closed_tiles()[(y * MAP_W + x) as usize] {
        return None;
    }
    match Realm::get().at(x, y) {
        b'=' | b':' | b'H' => Some(2),
        b'.' | b's' | b'a' => Some(5),
        b'c' => Some(7),
        b'%' => Some(9),
        _ => None,
    }
}

/// Cheapest tile route from `from` to `to`, excluding `from`. Falls back to
/// the goal alone (a straight walk) if the map leaves no way through.
pub(crate) fn route(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    if from == to {
        return Vec::new();
    }
    let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
    let inside = |(x, y): (i32, i32)| x >= 0 && y >= 0 && x < MAP_W && y < MAP_H;
    if !inside(from) || !inside(to) {
        return vec![to];
    }
    let mut best = vec![u32::MAX; (MAP_W * MAP_H) as usize];
    let mut prev = vec![usize::MAX; (MAP_W * MAP_H) as usize];
    let mut open = BinaryHeap::new();
    best[idx(from)] = 0;
    open.push(Reverse((0u32, idx(from))));
    while let Some(Reverse((cost, at))) = open.pop() {
        if at == idx(to) {
            break;
        }
        if cost > best[at] {
            continue;
        }
        let (x, y) = ((at as i32) % MAP_W, (at as i32) / MAP_W);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            let step = if (nx, ny) == to {
                Some(2)
            } else {
                step_cost(nx, ny)
            };
            let Some(step) = step else { continue };
            let n = idx((nx, ny));
            let c = cost + step;
            if c < best[n] {
                best[n] = c;
                prev[n] = at;
                open.push(Reverse((c, n)));
            }
        }
    }
    if best[idx(to)] == u32::MAX {
        return vec![to];
    }
    let mut path = Vec::new();
    let mut at = idx(to);
    while at != idx(from) {
        path.push(((at as i32) % MAP_W, (at as i32) / MAP_W));
        at = prev[at];
    }
    path.reverse();
    path
}

/// The HUD item for a kind of work.
pub(crate) fn tool_for(activity: RealmActivity) -> Option<Tool> {
    match activity {
        RealmActivity::Forge => Some(Tool::Hammer),
        RealmActivity::Study => Some(Tool::Book),
        RealmActivity::Chronicle => Some(Tool::Quill),
        RealmActivity::Dispatch => Some(Tool::Scroll),
        RealmActivity::Council => Some(Tool::Sword),
        RealmActivity::Memory => Some(Tool::Candle),
        RealmActivity::Research => Some(Tool::Lens),
        RealmActivity::Errand => None,
    }
}

/// Where an adventure's region sits on the map.
pub(crate) fn region_place(region: Region) -> Option<Place> {
    match region {
        Region::CastleTown => None,
        Region::TheMines => Some(Place::Mines),
        Region::DarkForest => Some(Place::DarkForest),
        Region::Swamp => Some(Place::Swamp),
        Region::DragonKeep => Some(Place::DragonKeep),
        Region::Homecoming => Some(Place::Fields),
    }
}

/// The nearest signal ink to a banner colour.
fn signal_ink(rgb_banner: (u8, u8, u8)) -> char {
    const INKS: [char; 15] = [
        '0', '1', '2', '3', 'w', 'a', '4', '@', '5', '6', '$', 'c', '9', '8', '7',
    ];
    let (r, g, b) = rgb_banner;
    let mut best = (u32::MAX, '5');
    for (i, &ink) in INKS.iter().enumerate() {
        let p = rgb(PALETTE[SIGNAL_BANK * BANK + i]);
        let d = (p[0] as i32 - r as i32).pow(2) as u32
            + (p[1] as i32 - g as i32).pow(2) as u32
            + (p[2] as i32 - b as i32).pow(2) as u32;
        if d < best.0 {
            best = (d, ink);
        }
    }
    best.1
}

/// Council robes, one per seat in order of arrival.
const ROBES: [char; 6] = ['1', '2', '@', '5', '3', '7'];

/// Day clock to ambient: the approved dusk mood is the brightest the realm
/// gets; night sinks below it.
pub(crate) fn ambient_for(daylight: f32) -> f32 {
    let t = ((daylight - 0.55) / 0.45).clamp(0.0, 1.0);
    DUSK - 0.14 * (1.0 - t)
}

impl World {
    /// The place the pixel knight walks to: the quest's region while an
    /// adventure is out, the quintain at the Lists between loop rounds,
    /// otherwise the landmark the work is happening in.
    pub(crate) fn overworld_goal(&self) -> Place {
        if self.loop_active {
            if let Some(place) = region_place(self.quest.region()) {
                return place;
            }
            if self.target == Building::Keep {
                return Place::Lists;
            }
        }
        Place::of_building(self.target)
    }

    /// Advance the pixel knight one world tick.
    pub(crate) fn tick_overworld(&mut self) {
        let goal = self.overworld_goal();
        self.overworld.toward(goal);
    }

    /// Everything the pixel map shows, read from the live world.
    pub(crate) fn overworld_scene(&self) -> Scene {
        let mut s = Scene::resting();
        s.tier = self.level();
        let work = self.latest_active_work();
        s.active = work.map(|w| Place::of_building(w.landmark));
        s.tool = work.and_then(|w| tool_for(w.activity));
        s.knight = self.overworld.knight();
        s.camera = self.overworld.camera();
        let council = self
            .active_work()
            .filter(|w| w.landmark == Building::RoundTable)
            .count()
            .min(6);
        s.council = (0..council)
            .map(|i| (i * 6 / council.max(1), ROBES[i]))
            .collect();
        if self.loop_active {
            let max = self.loop_budget.map_or(0, |b| b.max_iters as u32);
            s.quest = Some((
                self.loop_iteration as u32,
                max.max(self.loop_iteration as u32),
            ));
            s.region = region_place(self.quest.region());
        }
        s.chapel_lit = self.memory_health == crate::knowledge::memory::store::MemoryHealth::Healthy;
        if let Some(village) = &self.village {
            for (lit, (_, up)) in s.cottages.iter_mut().zip(&village.heads_up) {
                *lit = *up;
            }
            s.forge_hot = village.forge_up && village.training;
        }
        s.wards = self
            .districts
            .iter()
            .take(3)
            .map(|d| Ward {
                name: d.name.clone(),
                banner: signal_ink(d.banner_color),
                lit: false,
            })
            .collect();
        s.muster = self
            .muster
            .iter()
            .map(|u| Soldier {
                kind: u.kind,
                state: u.state,
            })
            .collect();
        s.weather = if self.tick < self.storm_until {
            Weather::Storm
        } else if self.tick < self.rainbow_until {
            Weather::Rainbow
        } else if self.raining() {
            Weather::Rain
        } else if self.budget_thin() {
            Weather::Clouds
        } else if self.outcome_weather() == super::super::life::Weather::Drizzle {
            Weather::Drizzle
        } else {
            Weather::Fair
        };
        s.fireworks = self.tick < self.firework_until;
        s.ambient = ambient_for(self.hearth.daylight())
            - match s.weather {
                Weather::Rain | Weather::Storm => 0.05,
                Weather::Clouds | Weather::Drizzle => 0.03,
                _ => 0.0,
            };
        let quest = &self.quest;
        s.party = self.overworld.followers(usize::from(quest.party()).min(4));
        s.chests = (quest.treasures(), quest.empty_chests());
        s.dragon = self.loop_active && quest.region() == Region::DragonKeep;
        s.wisps = self.loop_active && quest.region() == Region::Swamp;
        if let Some(duel) = &self.overworld_duel {
            let charging = duel.states.iter().all(|&st| st == SoldierState::Running);
            let score = |seat: &String| self.lists_tally.get(seat).copied().unwrap_or(0);
            s.joust = Some(Joust {
                red: duel.seats[0].clone(),
                blue: duel.seats[1].clone(),
                red_score: score(&duel.seats[0]),
                blue_score: score(&duel.seats[1]),
                charge: if charging {
                    (self.tick.saturating_sub(duel.since) % 96) as f32 / 96.0
                } else {
                    1.0
                },
            });
        }
        s.tick = self.tick as u32;
        s
    }
}

impl World {
    /// The glass picture for `sequence`, rendered by `paint` only when the
    /// source frame changed since the map last asked.
    fn glass_picture(&self, sequence: u64, paint: impl FnOnce() -> Img) -> std::sync::Arc<Img> {
        if let Some((key, img)) = self.overworld_glass.borrow().as_ref()
            && *key == sequence
        {
            return std::sync::Arc::clone(img);
        }
        let img = std::sync::Arc::new(paint());
        *self.overworld_glass.borrow_mut() = Some((sequence, std::sync::Arc::clone(&img)));
        img
    }

    /// The arrival glass: the place's authored painting.
    pub(crate) fn overworld_plate_glass(&self, building: Building) -> Glass {
        let place = Place::of_building(building);
        let sequence = 0x9_1a7e ^ building as u64;
        let picture = self.glass_picture(sequence, || {
            let plate = super::super::ambient::plate(building);
            picture_from_rgba(plate.as_raw(), plate.width(), plate.height())
        });
        Glass {
            anchor: place,
            title: place.label().to_string(),
            live: false,
            picture,
            sequence,
        }
    }

    /// The travel glass: the Dotmax ride from the saddle, heading for
    /// `destination`. It steps at the map's own pace.
    pub(crate) fn overworld_ride_glass(&self, destination: Building) -> Glass {
        let place = Place::of_building(destination);
        let sequence = self.cinematic_key() ^ (self.tick / 6).rotate_left(17);
        let picture = self.glass_picture(sequence, || {
            let (map, view) = self.travel_scene();
            let mut frame = super::super::world3d::render_region_frame(
                self.quest(),
                &map,
                &view,
                0.0,
                self.tick / super::super::world3d::region::WISP_TICKS,
                (GLASS_W * 2) as u32,
                (GLASS_H * 2) as u32,
            );
            super::super::ride::composite_rider_overlay(
                &mut frame,
                super::super::cinematics::rider_frame_key(self),
            );
            picture_from_rgba(frame.as_raw(), frame.width(), frame.height())
        });
        Glass {
            anchor: place,
            title: format!("ride > {}", place.label()),
            live: true,
            picture,
            sequence,
        }
    }
}

/// A two-seat fan-out stage, fought at the Lists.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Duel {
    stage_id: u64,
    seats: [String; 2],
    states: [SoldierState; 2],
    since: u64,
}

impl World {
    /// Mirror the published fan-out stage onto the Lists: exactly two seats
    /// is a duel; a seat coming back with a result scores for its label in
    /// the session tally. Any other stage ends the duel; the tally stays.
    pub(crate) fn note_duel(
        &mut self,
        stage_id: u64,
        agents: &[String],
        states: &[crate::ui::viz::agentviz::SeatState],
    ) {
        if agents.len() != 2 {
            self.overworld_duel = None;
            return;
        }
        if self
            .overworld_duel
            .as_ref()
            .is_none_or(|d| d.stage_id != stage_id)
        {
            self.overworld_duel = Some(Duel {
                stage_id,
                seats: [agents[0].clone(), agents[1].clone()],
                states: [SoldierState::Running; 2],
                since: self.tick,
            });
        }
        let Some(duel) = self.overworld_duel.as_mut() else {
            return;
        };
        for i in 0..2 {
            let now = states.get(i).map_or(SoldierState::Running, soldier_state);
            if now == SoldierState::Returned && duel.states[i] != SoldierState::Returned {
                *self.lists_tally.entry(duel.seats[i].clone()).or_default() += 1;
            }
            duel.states[i] = now;
        }
    }
}

/// A muster seat's state, kept beside its braille glyph.
pub(crate) fn soldier_state(state: &crate::ui::viz::agentviz::SeatState) -> SoldierState {
    use crate::ui::viz::agentviz::SeatState;
    match state {
        SeatState::Running => SoldierState::Running,
        SeatState::Returned => SoldierState::Returned,
        SeatState::Failed => SoldierState::Failed,
        SeatState::Cut => SoldierState::Cut,
    }
}
