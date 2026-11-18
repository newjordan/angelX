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

use super::super::adventure::Region;
use super::super::{Building, RealmActivity, Sky, World};
use super::castles::{self, RiderPose};
use super::glass::{GLASS_H, GLASS_W, Glass, picture_from_rgba};
use super::ink::Img;
use super::ink::{BANK, PALETTE, SIGNAL_BANK, rgb};
use super::kit::Tool;
use super::light::DUSK;
use super::map::{MAP_H, MAP_W, Place, TILE};
use super::scene::{Joust, Knight, Scene, Soldier, SoldierState, Ward, Weather};
use angel_delve::world::houses::{HouseId, Serving};
use crate::telemetry::MotionMode;
use angel_delve::world::kingdom::step_cost;

fn overworld_motion(mode: MotionMode) -> angel_delve::render::overworld::Motion {
    use angel_delve::render::overworld::Motion;
    match mode {
        MotionMode::Full => Motion::Full,
        MotionMode::Reduced => Motion::Reduced,
        MotionMode::Off => Motion::Off,
    }
}

const PACE: f32 = 1.5;
const DEAD_X: f32 = 40.0;
const DEAD_Y: f32 = 28.0;
const FOCUS_LIFT: f32 = 8.0;

pub fn follow(cam: (f32, f32), focus: (f32, f32)) -> (f32, f32) {
    follow_within(cam, focus, (DEAD_X, DEAD_Y))
}

pub(super) fn follow_within(
    cam: (f32, f32),
    focus: (f32, f32),
    (dead_x, dead_y): (f32, f32),
) -> (f32, f32) {
    let axis = |c: f32, f: f32, dead: f32| {
        if f - c > dead {
            f - dead
        } else if c - f > dead {
            f + dead
        } else {
            c
        }
    };
    (axis(cam.0, focus.0, dead_x), axis(cam.1, focus.1, dead_y))
}

const TRAIL: usize = 64;
const FOLLOW_GAP: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Goal {
    Place(Place),
    Spot((i32, i32)),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Walker {
    x: f32,
    y: f32,
    goal: Goal,
    path: VecDeque<(i32, i32)>,
    trail: VecDeque<(f32, f32)>,
    cam: (f32, f32),
    stride: f32,
    shown: Shown,
    serving: Serving,
    served: bool,
    riders: Vec<Rider>,
    watch: Vec<WatchGuard>,
    watch_house: Option<HouseId>,
}

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    knight: Knight,
    cam: (f32, f32),
    party: Vec<(f32, f32)>,
    riders: Vec<RiderPose>,
    watch: Vec<RiderPose>,
}

#[derive(Clone, Debug, PartialEq)]
struct Rider {
    house: HouseId,
    x: f32,
    y: f32,
    path: VecDeque<(i32, i32)>,
    stride: f32,
    aim: Option<(i32, i32)>,
    facing_left: bool,
}

impl Rider {
    fn at(house: HouseId, (x, y): (f32, f32)) -> Rider {
        Rider {
            house,
            x,
            y,
            path: VecDeque::new(),
            stride: PACE,
            aim: None,
            facing_left: false,
        }
    }

    fn tile(&self) -> (i32, i32) {
        (
            (self.x / TILE as f32).floor() as i32,
            (self.y / TILE as f32).floor() as i32,
        )
    }

    fn homebound(&self) -> bool {
        self.aim.is_none()
    }

    fn set_out(&mut self, goal: (i32, i32)) {
        let path = route(self.tile(), goal);
        self.stride = (path.len() as f32 * TILE as f32 / 160.0).max(PACE);
        self.path = path.into();
    }

    fn ride(&mut self) {
        let mut stride = self.stride;
        while stride > 0.0 {
            let Some(&next) = self.path.front() else {
                break;
            };
            let (gx, gy) = feet(next);
            let (dx, dy) = (gx - self.x, gy - self.y);
            if dx.abs() > 0.01 {
                self.facing_left = dx < 0.0;
            }
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

    fn pose(&self) -> RiderPose {
        RiderPose {
            house: self.house,
            x: self.x,
            y: self.y,
            walking: !self.path.is_empty(),
            facing_left: self.facing_left,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct WatchGuard {
    rider: Rider,
    dest: (i32, i32),
}

impl Default for Walker {
    fn default() -> Walker {
        let home = Knight::at_place(Place::Keep);
        Walker {
            x: home.x,
            y: home.y,
            goal: Goal::Place(Place::Keep),
            path: VecDeque::new(),
            trail: VecDeque::new(),
            cam: (home.x, home.y - FOCUS_LIFT),
            stride: PACE,
            shown: Shown {
                knight: home,
                cam: (home.x, home.y - FOCUS_LIFT),
                party: Vec::new(),
                riders: Vec::new(),
                watch: Vec::new(),
            },
            serving: Serving::default(),
            served: false,
            riders: Vec::new(),
            watch: Vec::new(),
            watch_house: None,
        }
    }
}

pub(super) fn feet((tx, ty): (i32, i32)) -> (f32, f32) {
    ((tx * TILE + TILE / 2) as f32, ((ty + 1) * TILE - 2) as f32)
}

impl Walker {
    pub fn moving(&self) -> bool {
        !self.path.is_empty() || self.shown.knight.walking
    }

    pub fn travelling_to(&self, goal: Place) -> bool {
        self.goal != Goal::Place(goal) || !self.path.is_empty() || self.shown.knight.walking
    }

    pub fn standing_at(&self, spot: (i32, i32)) -> bool {
        self.goal == Goal::Spot(spot) && self.path.is_empty() && !self.shown.knight.walking
    }

    pub fn toward(&mut self, goal: Place) {
        self.toward_goal(Goal::Place(goal));
    }

    pub fn toward_spot(&mut self, spot: (i32, i32)) {
        self.toward_goal(Goal::Spot(spot));
    }

    fn toward_goal(&mut self, goal: Goal) {
        if goal != self.goal {
            self.goal = goal;
            let tile = match goal {
                Goal::Place(place) => place.stand_world(),
                Goal::Spot(spot) => spot,
            };
            self.path = route(self.tile(), tile).into();
            self.stride = (self.path.len() as f32 * TILE as f32 / 160.0).max(PACE);
        }
        let before = (self.x, self.y);
        let mut stride = self.stride;
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
        self.tick_riders();
        self.tick_watch();
    }

    pub fn camera(&self) -> (f32, f32) {
        self.cam
    }

    pub fn shown_knight(&self) -> Knight {
        self.shown.knight
    }

    pub fn publish(&mut self) {
        self.shown = Shown {
            knight: self.knight(),
            cam: self.cam,
            party: self.followers(4),
            riders: self.riders.iter().map(Rider::pose).collect(),
            watch: self.watch.iter().map(|g| g.rider.pose()).collect(),
        };
    }

    pub fn serving(&self) -> &Serving {
        &self.serving
    }

    pub fn shown_riders(&self) -> &[RiderPose] {
        &self.shown.riders
    }

    pub fn shown_watch(&self) -> &[RiderPose] {
        &self.shown.watch
    }

    fn arrive_at(&mut self, (x, y): (f32, f32)) {
        (self.x, self.y) = (x, y);
        self.path.clear();
        self.trail.clear();
        self.goal = Goal::Spot((-1, -1));
    }

    pub fn serve(&mut self, now: &Serving) {
        if *now == self.serving {
            return;
        }
        let before = std::mem::replace(&mut self.serving, now.clone());
        self.post_watch(now);
        if before.lead != now.lead {
            let here = (self.x, self.y);
            if self.served
                && let Some(old) = before.lead
                && !now.seated.contains(&old)
            {
                let mut home = Rider::at(old, here);
                home.set_out(castles::stand(old));
                self.riders.push(home);
            }
            let start = match now.lead {
                Some(lead) => match self.riders.iter().position(|r| r.house == lead) {
                    Some(i) => {
                        let r = self.riders.remove(i);
                        (r.x, r.y)
                    }
                    None => feet(castles::stand(lead)),
                },
                None => feet(Place::Keep.stand_world()),
            };
            self.arrive_at(start);
            if self.served
                && let Some(old) = before.lead
                && now.seated.contains(&old)
            {
                self.riders.push(Rider::at(old, here));
            }
            self.served |= now.lead.is_some();
        }
        for r in &mut self.riders {
            if !r.homebound() && !now.seated.contains(&r.house) {
                r.aim = None;
                r.set_out(castles::stand(r.house));
            }
        }
        for &house in &now.seated {
            if Some(house) == now.lead {
                continue;
            }
            match self.riders.iter_mut().find(|r| r.house == house) {
                Some(r) => {
                    if r.homebound() {
                        r.aim = Some((-1, -1));
                    }
                }
                None => {
                    let mut r = Rider::at(house, feet(castles::stand(house)));
                    r.aim = Some((-1, -1));
                    self.riders.push(r);
                }
            }
        }
    }

    fn post_watch(&mut self, now: &Serving) {
        let duty = if now.turn { now.lead } else { None };
        if duty == self.watch_house {
            return;
        }
        self.watch.clear();
        self.watch_house = duty;
        let Some(lead) = duty else { return };
        let keep_gate = Place::Keep.stand_world();
        let castle_gate = castles::stand(lead);
        if keep_gate == castle_gate {
            return;
        }
        for (from, dest) in [(keep_gate, castle_gate), (castle_gate, keep_gate)] {
            let mut rider = Rider::at(lead, feet(from));
            rider.set_out(dest);
            rider.stride = PACE;
            self.watch.push(WatchGuard { rider, dest });
        }
    }

    fn tick_watch(&mut self) {
        let Some(lead) = self.watch_house else { return };
        let keep_gate = Place::Keep.stand_world();
        let castle_gate = castles::stand(lead);
        for guard in &mut self.watch {
            if guard.rider.path.is_empty() {
                if guard.rider.tile() == guard.dest {
                    guard.dest = if guard.dest == keep_gate {
                        castle_gate
                    } else {
                        keep_gate
                    };
                }
                guard.rider.set_out(guard.dest);
                guard.rider.stride = PACE;
            }
            guard.rider.ride();
        }
    }

    fn goal_tile(&self) -> (i32, i32) {
        match self.goal {
            Goal::Place(place) => place.stand_world(),
            Goal::Spot(spot) => spot,
        }
    }

    fn tick_riders(&mut self) {
        let goal = self.goal_tile();
        let retinue = self.riders.iter().filter(|r| !r.homebound()).count();
        if retinue > 0
            && goal.0 >= 0
            && self
                .riders
                .iter()
                .any(|r| r.aim.is_some_and(|aim| aim != goal))
        {
            let spots = muster(goal, retinue);
            let mut spot = spots.into_iter();
            for r in self.riders.iter_mut().filter(|r| !r.homebound()) {
                r.aim = Some(goal);
                if let Some(tile) = spot.next() {
                    r.set_out(tile);
                }
            }
        }
        for r in &mut self.riders {
            r.ride();
        }
        self.riders
            .retain(|r| !(r.homebound() && r.path.is_empty()));
    }

    pub fn followers(&self, n: usize) -> Vec<(f32, f32)> {
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

    pub fn knight(&self) -> Knight {
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

pub fn route(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
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

pub(super) fn muster(goal: (i32, i32), n: usize) -> Vec<(i32, i32)> {
    let idx = |(x, y): (i32, i32)| (y * MAP_W + x) as usize;
    let inside = |(x, y): (i32, i32)| x >= 0 && y >= 0 && x < MAP_W && y < MAP_H;
    if !inside(goal) || n == 0 {
        return Vec::new();
    }
    let mut best = std::collections::HashMap::new();
    let mut open = BinaryHeap::new();
    let mut out = Vec::new();
    best.insert(idx(goal), 0u32);
    open.push(Reverse((0u32, goal.1, goal.0)));
    while let Some(Reverse((cost, y, x))) = open.pop() {
        if best.get(&idx((x, y))).is_some_and(|&c| c < cost) {
            continue;
        }
        if (x, y) != goal {
            out.push((x, y));
            if out.len() == n {
                break;
            }
        }
        for (dx, dy) in [(0, 1), (-1, 0), (1, 0), (0, -1)] {
            let next = (x + dx, y + dy);
            let Some(step) = step_cost(next.0, next.1) else {
                continue;
            };
            let c = cost + step;
            if best.get(&idx(next)).is_none_or(|&old| c < old) {
                best.insert(idx(next), c);
                open.push(Reverse((c, next.1, next.0)));
            }
        }
    }
    out
}

pub fn place_of_building(b: Building) -> Place {
    match b {
        Building::Keep => Place::Keep,
        Building::Gatehouse => Place::Gatehouse,
        Building::Rookery => Place::Rookery,
        Building::Scriptorium => Place::Scriptorium,
        Building::Smithy => Place::Smithy,
        Building::Chapel => Place::Chapel,
        Building::RoundTable => Place::RoundTable,
        Building::Observatory => Place::Observatory,
    }
}

pub fn tool_for(activity: RealmActivity) -> Option<Tool> {
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

pub fn region_place(region: Region) -> Option<Place> {
    match region {
        Region::CastleTown => None,
        Region::TheMines => Some(Place::Mines),
        Region::DarkForest => Some(Place::DarkForest),
        Region::Swamp => Some(Place::Swamp),
        Region::DragonKeep => Some(Place::DragonKeep),
        Region::Homecoming => Some(Place::Fields),
    }
}

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

const ROBES: [char; 6] = ['1', '2', '@', '5', '3', '7'];

pub fn ambient_for(daylight: f32) -> f32 {
    let t = ((daylight - 0.55) / 0.45).clamp(0.0, 1.0);
    DUSK - 0.14 * (1.0 - t)
}

impl World {
    pub fn graph_knight_travelling(&self) -> bool {
        self.graph_destination && self.overworld.travelling_to(Place::Fields)
    }
    pub fn visit_overworld(&mut self, name: &str) -> Option<&'static str> {
        let normalize = |text: &str| {
            text.chars()
                .filter(char::is_ascii_alphanumeric)
                .map(|ch| ch.to_ascii_lowercase())
                .collect::<String>()
        };
        let name = normalize(name);
        if matches!(
            name.as_str(),
            "tower" | "scryingtower" | "wizardstower" | "wizardtower"
        ) {
            self.visit_scrying_tower();
            return Some(super::super::ambient::SCRYING_TOWER_LABEL);
        }
        if let Some(id) = angel_delve::world::houses::find(&name) {
            let (x, y, w, h) = castles::footprint(id);
            if self.visiting_scrying_tower() {
                self.interior = None;
            }
            let label = angel_delve::world::houses::get(id).castle.as_str();
            self.overworld_view = Some((
                (x as f32 + w as f32 / 2.0) * TILE as f32,
                (y as f32 + h as f32 / 2.0) * TILE as f32,
                label,
            ));
            self.school_room = None;
            return Some(label);
        }
        if let Some(room) = super::school::Room::parse(&name) {
            let (x, y, w, h) = Place::School.footprint_world();
            self.overworld_view = Some((
                (x as f32 + w as f32 / 2.0) * TILE as f32,
                (y as f32 + h as f32 / 2.0) * TILE as f32,
                room.label(),
            ));
            self.interior = None;
            self.school_room = Some(room);
            return Some(room.label());
        }
        if matches!(name.as_str(), "garden" | "graphgarden" | "crops") {
            let (x, y) = super::garden::centre();
            if self.visiting_scrying_tower() {
                self.interior = None;
            }
            self.overworld_view = Some((x, y, "GRAPH GARDEN"));
            self.school_room = None;
            return Some("GRAPH GARDEN");
        }
        let place = match name.as_str() {
            "artisan" | "artisans" | "artisanquarter" => Place::ArtisanQuarter,
            "colosseum" | "arena" => Place::Colosseum,
            "tournament" | "knights" => Place::Tournament,
            "fields" => Place::Fields,
            "wards" => Place::Wards,
            "forest" => Place::DarkForest,
            "table" => Place::RoundTable,
            "school" | "magic" | "sloptomizer" | "magicschool" => Place::School,
            _ => Place::ALL.into_iter().find(|place| {
                normalize(place.label()) == name
                    || normalize(place.label().trim_start_matches("THE ")) == name
            })?,
        };
        let (x, y, w, h) = place.footprint_world();
        if self.visiting_scrying_tower() {
            self.interior = None;
        }
        self.overworld_view = Some((
            (x as f32 + w as f32 / 2.0) * TILE as f32,
            (y as f32 + h as f32 / 2.0) * TILE as f32,
            place.label(),
        ));
        self.school_room = None;
        if place == Place::School {
            self.interior = None;
        }
        Some(place.label())
    }

    pub fn toggle_adventure_map(&mut self) -> bool {
        let Some(place) = region_place(self.quest().region()) else {
            return false;
        };
        if self.inside_interior() || self.graph_visiting() {
            return false;
        }
        match self.overworld_view_label() {
            None => self.visit_overworld(place.label()).is_some(),
            Some(label) if label == place.label() => {
                self.follow_overworld();
                true
            }
            _ => false,
        }
    }

    pub fn follow_overworld(&mut self) {
        if self.visiting_scrying_tower() {
            self.interior = None;
        }
        self.overworld_view = None;
        self.school_room = None;
    }

    pub fn note_school(&mut self, snapshot: super::school::SchoolSnapshot) {
        self.school = snapshot;
    }

    pub fn overworld_view_label(&self) -> Option<&'static str> {
        self.overworld_view.map(|(_, _, label)| label)
    }

    pub fn visiting_school(&self) -> bool {
        self.school_room.is_some() || self.overworld_view_label() == Some(Place::School.label())
    }

    pub fn overworld_goal(&self) -> Place {
        if self.graph_destination {
            return Place::Fields;
        }
        if !self.loop_active && self.overworld_deeds.trial_underway() {
            return Place::Lists;
        }
        if self.loop_active {
            if let Some(place) = region_place(self.quest.region()) {
                return place;
            }
            if self.target == Building::Keep {
                return Place::Lists;
            }
        }
        place_of_building(self.target)
    }

    pub fn at_delve_gate(&self) -> bool {
        self.delve_called && self.overworld.standing_at(super::wishes::delve_stand())
    }

    pub fn note_serving(&mut self, serving: &Serving) {
        self.overworld.serve(serving);
    }

    pub fn overworld_home(&self) -> Option<(i32, i32)> {
        let serving = self.overworld.serving();
        let lead = serving.lead?;
        let resting = !serving.turn
            && !self.loop_active
            && !self.graph_destination
            && self.latest_active_work().is_none()
            && self.overworld_goal() == Place::Keep;
        resting.then(|| castles::stand(lead))
    }

    pub fn tick_overworld(&mut self) {
        self.overworld_deeds.step(self.tick);
        if self.delve_called {
            self.overworld.toward_spot(super::wishes::delve_stand());
        } else if let Some(home) = self.overworld_home() {
            self.overworld.toward_spot(home);
        } else {
            let goal = self.overworld_goal();
            self.overworld.toward(goal);
        }
        self.overworld.publish();
        self.tick_visitor();
    }

    pub fn overworld_scene(&self) -> Scene {
        self.overworld_scene_moving(MotionMode::Full)
    }

    pub fn overworld_scene_moving(&self, motion: MotionMode) -> Scene {
        let mut s = Scene::resting();
        s.outcome_motion = overworld_motion(motion);
        s.tier = self.level();
        let work = self.latest_active_work();
        s.active = work.map(|w| place_of_building(w.landmark));
        s.tool = work.and_then(|w| tool_for(w.activity));
        if !self.loop_active && self.overworld_deeds.trial_underway() {
            s.active = Some(Place::Lists);
            s.tool = Some(Tool::Sword);
        }
        s.wayfarers = self.overworld_deeds.wayfarers();
        if angel_delve::world::plan::tabula() {
            s.wayfarers.clear();
        }
        s.record = self.overworld_deeds.record().clone();
        s.sparks = self.overworld_deeds.sparks(self.tick);
        s.outcomes = self.overworld_outcomes.shown(self.tick);
        s.stargazing = self.overworld_deeds.stargazing();
        let mut garden = self.graph_garden.frame(self.tick);
        garden.motion = motion;
        s.overlay = Some(std::sync::Arc::new(super::MiniViz { garden }));
        s.school = self.school;
        s.school_room = self.school_room;
        s.wishes = crate::realm_standing::standing_wishes();
        s.delve_lit = self.delve_lit || self.delve_called;
        s.delve_boons = if s.delve_lit { self.delve_boons } else { 0 };
        if self.graph_garden.working() {
            s.active = Some(Place::Fields);
            s.tool = Some(Tool::Quill);
        }
        s.knight = self.overworld.shown.knight;
        s.visitor = self.shown_visitor();
        let serving = self.overworld.serving();
        s.march = castles::March {
            raised: serving.houses().collect(),
            lead: serving.lead,
            riders: self.overworld.shown_riders().to_vec(),
            watch: self.overworld.shown_watch().to_vec(),
        };
        s.camera = self
            .overworld_view
            .map(|(x, y, _)| (x, y))
            .or_else(|| self.visitor_camera())
            .unwrap_or(self.overworld.shown.cam);
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
        s.chapel_lit = self.memory_health == crate::telemetry::MemoryHealth::Healthy;
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
        s.weather = match self.sky() {
            Sky::Storm => Weather::Storm,
            Sky::Rainbow => Weather::Rainbow,
            Sky::Rain => Weather::Rain,
            Sky::Clouds => Weather::Clouds,
            Sky::Drizzle => Weather::Drizzle,
            Sky::Fair => Weather::Fair,
        };
        s.fireworks = self.tick < self.firework_until;
        s.ambient = ambient_for(self.hearth.daylight())
            - match s.weather {
                Weather::Rain | Weather::Storm => 0.05,
                Weather::Clouds | Weather::Drizzle => 0.03,
                _ => 0.0,
            };
        let quest = &self.quest;
        s.party = self
            .overworld
            .shown
            .party
            .iter()
            .take(usize::from(quest.party()).min(4))
            .copied()
            .collect();
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

    pub fn overworld_plate_glass(&self, building: Building) -> Glass {
        let place = place_of_building(building);
        let sequence = 0x9_1a7e ^ building as u64;
        let picture = self.glass_picture(sequence, || {
            if building == Building::Observatory {
                let frame = super::super::ambient::frame(building, Default::default());
                picture_from_rgba(frame.as_raw(), frame.width(), frame.height())
            } else {
                let plate = super::super::ambient::plate(building);
                picture_from_rgba(plate.as_raw(), plate.width(), plate.height())
            }
        });
        Glass {
            anchor: place,
            title: if building == Building::Observatory {
                super::super::ambient::SCRYING_TOWER_LABEL.to_string()
            } else {
                place.label().to_string()
            },
            live: false,
            picture,
            sequence,
        }
    }

    pub fn overworld_ride_glass(&self, destination: Building) -> Glass {
        let place = place_of_building(destination);
        let sequence = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            ("ride-glass", destination, self.tick).hash(&mut h);
            h.finish()
        };
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

#[derive(Clone, Debug, PartialEq)]
pub struct Duel {
    stage_id: u64,
    seats: [String; 2],
    states: [SoldierState; 2],
    since: u64,
}

impl World {
    pub fn note_duel(
        &mut self,
        stage_id: u64,
        agents: &[String],
        states: &[crate::telemetry::SeatState],
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

pub fn soldier_state(state: &crate::telemetry::SeatState) -> SoldierState {
    use crate::telemetry::SeatState;
    match state {
        SeatState::Running => SoldierState::Running,
        SeatState::Returned => SoldierState::Returned,
        SeatState::Failed => SoldierState::Failed,
        SeatState::Cut => SoldierState::Cut,
    }
}

#[cfg(test)]
#[path = "../../../tests/world_viz/overworld__navigation_tests.rs"]
mod navigation_tests;
