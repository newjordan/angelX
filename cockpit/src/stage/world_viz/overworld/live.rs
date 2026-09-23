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
use super::scene::{Knight, Scene, Soldier, SoldierState, Ward, Weather};

/// Walking pace in world pixels per world tick (the world ticks at 40 Hz).
const PACE: f32 = 1.5;

/// The knight's walk across the realm.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Walker {
    x: f32,
    y: f32,
    goal: Place,
    path: VecDeque<(i32, i32)>,
}

impl Default for Walker {
    fn default() -> Walker {
        let home = Knight::at_place(Place::Keep);
        Walker {
            x: home.x,
            y: home.y,
            goal: Place::Keep,
            path: VecDeque::new(),
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
        let mut s = Scene::resting(&self.town_name);
        s.renown = self.renown;
        s.verified = self.verified_wins;
        s.tier = self.level();
        let work = self.latest_active_work();
        s.active = work.map(|w| Place::of_building(w.landmark));
        s.tool = work.and_then(|w| tool_for(w.activity));
        s.activity = self.activity.clone();
        s.knight = self.overworld.knight();
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
        s.ambient = ambient_for(self.hearth.daylight());
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
