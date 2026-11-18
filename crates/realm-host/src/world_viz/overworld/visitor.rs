//! The visitor: the operator's own figure on the map, walked by hand while
//! the harness's knight goes on with his work.
//!
//! He keeps to the ground the knight walks (roads, meadow, swamp) and stops
//! at trees, rock, water and every structure. The realm's doors are tiles it
//! already names: each place's stand, each castle's gate, the Delve's stair
//! (from across its stream). Standing at one names where it leads; going in
//! is the cockpit's to do (`app::control::overworld_walk`), since what opens
//! is not the map.

use std::collections::VecDeque;

use super::super::{Building, World};
use super::View;
use super::ink::Img;
use super::kit;
use super::live::{feet, follow_within, muster, route};
use super::map::{MAP_H, MAP_W, Place, TILE};
use super::scene::Scene;
use angel_delve::world::houses::HouseId;
use angel_delve::world::kingdom::step_cost;

pub use angel_delve::world::doors::{Door, doors, walkable};
pub use angel_delve::render::overworld::VisitorPose;

const PACE: f32 = 1.25;
const FOOT_W: f32 = 4.0;
const FOOT_H: f32 = 3.0;
const NUDGE: i32 = 8;
const FOCUS_LIFT: f32 = 8.0;
const DEAD: (f32, f32) = (24.0, 12.0);
const NEAR: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opens {
    Room(Building),
    Formation,
    Library,
    Tower,
    Lists,
    Stables,
    School,
    House(HouseId),
    Delve,
    College,
}

pub trait DoorOpens {
    fn opens(self) -> Option<Opens>;
}

impl DoorOpens for Door {
    fn opens(self) -> Option<Opens> {
        Some(match self {
            Door::Place(Place::Keep) => Opens::Room(Building::Keep),
            Door::Place(Place::Gatehouse) => Opens::Room(Building::Gatehouse),
            Door::Place(Place::Rookery) => Opens::Room(Building::Rookery),
            Door::Place(Place::Smithy) => Opens::Room(Building::Smithy),
            Door::Place(Place::Chapel) => Opens::Room(Building::Chapel),
            Door::Place(Place::RoundTable) => Opens::Formation,
            Door::Place(Place::Scriptorium) => Opens::Library,
            Door::Place(Place::Observatory) => Opens::Tower,
            Door::Place(Place::Lists) => Opens::Lists,
            Door::Place(Place::Tournament) => Opens::Stables,
            Door::Place(Place::School) => Opens::School,
            Door::Castle(id) => Opens::House(id),
            Door::Delve => Opens::Delve,
            Door::College => Opens::College,
            Door::Place(_) => return None,
        })
    }
}

fn landing(tile: (i32, i32), reach: i32) -> Option<(i32, i32)> {
    if walkable(tile) {
        return Some(tile);
    }
    (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| (tile.0 + dx, tile.1 + dy)))
        .filter(|&t| walkable(t))
        .min_by_key(|&(x, y)| ((x - tile.0).pow(2) + (y - tile.1).pow(2), y, x))
}

fn tile_of(x: f32, y: f32) -> (i32, i32) {
    (
        (x / TILE as f32).floor() as i32,
        (y / TILE as f32).floor() as i32,
    )
}

pub fn door_near(x: f32, y: f32) -> Option<Door> {
    let (tx, ty) = tile_of(x, y);
    let far = |&&(door, _): &&((i32, i32), Door)| {
        let (fx, fy) = feet(door);
        (fx - x).powi(2) + (fy - y).powi(2)
    };
    doors()
        .iter()
        .filter(|((dx, dy), _)| (dx - tx).abs() + (dy - ty).abs() <= 1)
        .min_by(|a, b| far(a).total_cmp(&far(b)))
        .map(|&(_, door)| door)
}

fn clear(x: f32, y: f32) -> bool {
    [
        (x - FOOT_W, y - FOOT_H),
        (x + FOOT_W - 1.0, y - FOOT_H),
        (x - FOOT_W, y),
        (x + FOOT_W - 1.0, y),
    ]
    .into_iter()
    .all(|(px, py)| {
        let (tx, ty) = tile_of(px, py);
        step_cost(tx, ty).is_some()
    })
}

pub fn click_goal(x: f32, y: f32) -> Option<(i32, i32)> {
    let tile = tile_of(x, y);
    let inside = |(fx, fy, fw, fh): (i32, i32, i32, i32)| {
        tile.0 >= fx && tile.1 >= fy && tile.0 < fx + fw && tile.1 < fy + fh
    };
    if !walkable(tile)
        && let Some(&(stand, _)) = doors().iter().find(|(_, door)| inside(door.footprint()))
    {
        return Some(stand);
    }
    landing(tile, NEAR)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Visitor {
    x: f32,
    y: f32,
    out: bool,
    steer: (i8, i8),
    path: VecDeque<(i32, i32)>,
    facing_left: bool,
    moved: bool,
    cam: (f32, f32),
    shown: (VisitorPose, (f32, f32)),
}

impl Visitor {
    fn at((x, y): (f32, f32), cam: (f32, f32)) -> Visitor {
        let mut v = Visitor {
            x,
            y,
            out: false,
            steer: (0, 0),
            path: VecDeque::new(),
            facing_left: false,
            moved: false,
            cam,
            shown: (
                VisitorPose {
                    x,
                    y,
                    walking: false,
                    facing_left: false,
                },
                cam,
            ),
        };
        v.publish();
        v
    }

    fn pose(&self) -> VisitorPose {
        VisitorPose {
            x: self.x,
            y: self.y,
            walking: self.moved,
            facing_left: self.facing_left,
        }
    }

    fn publish(&mut self) {
        self.shown = (self.pose(), self.cam);
    }

    fn step(&mut self) {
        let before = (self.x, self.y);
        let (sx, sy) = (f32::from(self.steer.0), f32::from(self.steer.1));
        if self.steer != (0, 0) {
            self.path.clear();
            let pace = if sx != 0.0 && sy != 0.0 {
                PACE * std::f32::consts::FRAC_1_SQRT_2
            } else {
                PACE
            };
            self.shift(sx * pace, 0.0, sy == 0.0);
            self.shift(0.0, sy * pace, sx == 0.0);
        } else {
            self.follow_path();
        }
        if self.x != before.0 {
            self.facing_left = self.x < before.0;
        }
        self.moved = (self.x, self.y) != before;
        self.cam = follow_within(self.cam, (self.x, self.y - FOCUS_LIFT), DEAD);
    }

    fn shift(&mut self, dx: f32, dy: f32, assist: bool) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        if clear(self.x + dx, self.y + dy) {
            self.x += dx;
            self.y += dy;
            return;
        }
        if !assist {
            return;
        }
        for off in 1..=NUDGE {
            for sign in [-1.0, 1.0] {
                let o = off as f32 * sign;
                let (ox, oy) = if dx != 0.0 { (0.0, o) } else { (o, 0.0) };
                if clear(self.x + ox, self.y + oy) && clear(self.x + ox + dx, self.y + oy + dy) {
                    let slide = PACE.min(off as f32) * sign;
                    let (sx, sy) = if dx != 0.0 {
                        (0.0, slide)
                    } else {
                        (slide, 0.0)
                    };
                    if clear(self.x + sx, self.y + sy) {
                        self.x += sx;
                        self.y += sy;
                    }
                    return;
                }
            }
        }
    }

    fn follow_path(&mut self) {
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

    fn walk_to(&mut self, goal: (i32, i32)) -> bool {
        let here = tile_of(self.x, self.y);
        let mut path: VecDeque<_> = if goal == here {
            VecDeque::new()
        } else {
            let path = route(here, goal);
            let adjacent = (goal.0 - here.0).abs() + (goal.1 - here.1).abs() == 1;
            if path.last() != Some(&goal) || (path.len() == 1 && !adjacent) {
                return false;
            }
            path.into()
        };
        if (self.x, self.y) != feet(here) {
            path.push_front(here);
        }
        self.path = path;
        true
    }
}

impl World {
    pub fn walking(&self) -> bool {
        self.overworld_visitor.as_ref().is_some_and(|v| v.out)
    }

    pub fn start_walk(&mut self) {
        self.follow_overworld();
        let cam = self.overworld.camera();
        let knight = self.overworld.knight();
        let visitor = self.overworld_visitor.get_or_insert_with(|| {
            let here = tile_of(knight.x, knight.y);
            let spot = muster(here, 1).first().copied().unwrap_or(here);
            let spot = landing(spot, MAP_W.max(MAP_H)).unwrap_or(spot);
            Visitor::at(feet(spot), cam)
        });
        visitor.out = true;
        visitor.steer = (0, 0);
        visitor.path.clear();
        visitor.cam = follow_within(cam, (visitor.x, visitor.y - FOCUS_LIFT), DEAD);
        visitor.publish();
    }

    pub fn stop_walk(&mut self) {
        if let Some(v) = self.overworld_visitor.as_mut() {
            v.out = false;
            v.steer = (0, 0);
            v.path.clear();
            v.moved = false;
        }
    }

    pub fn steer_visitor(&mut self, steer: (i8, i8)) {
        if let Some(v) = self.overworld_visitor.as_mut().filter(|v| v.out) {
            v.steer = steer;
        }
    }

    pub fn walk_visitor_to(&mut self, x: f32, y: f32) -> bool {
        let Some(goal) = click_goal(x, y) else {
            return false;
        };
        self.overworld_visitor
            .as_mut()
            .filter(|v| v.out)
            .is_some_and(|v| v.walk_to(goal))
    }

    pub fn visitor_door(&self) -> Option<Door> {
        let pose = self.shown_visitor()?;
        door_near(pose.x.round(), pose.y.round())
    }

    pub fn visitor_busy(&self) -> bool {
        self.overworld_visitor.as_ref().is_some_and(|v| {
            v.out && (v.steer != (0, 0) || !v.path.is_empty() || v.moved || v.shown.0 != v.pose())
        })
    }

    pub(super) fn tick_visitor(&mut self) {
        if let Some(v) = self.overworld_visitor.as_mut().filter(|v| v.out) {
            v.step();
            v.publish();
        }
    }

    pub(super) fn shown_visitor(&self) -> Option<VisitorPose> {
        self.overworld_visitor
            .as_ref()
            .filter(|v| v.out)
            .map(|v| v.shown.0)
    }

    #[cfg(test)]
    pub fn stand_visitor_for_test(&mut self, opens: Opens) {
        let &(tile, _) = doors()
            .iter()
            .find(|(_, door)| door.opens() == Some(opens))
            .expect("a door opens it");
        let v = self
            .overworld_visitor
            .as_mut()
            .filter(|v| v.out)
            .expect("the visitor is out");
        (v.x, v.y) = feet(tile);
        v.path.clear();
        v.publish();
    }

    pub(super) fn visitor_camera(&self) -> Option<(f32, f32)> {
        self.overworld_visitor
            .as_ref()
            .filter(|v| v.out)
            .map(|v| v.shown.1)
    }
}

pub(super) fn paint_cue(cv: &mut Img, scene: &Scene, view: View) {
    let Some(v) = scene.visitor else {
        return;
    };
    let (img, gap) = match door_near(v.x, v.y) {
        Some(door) => (kit::door_sign(&door.sign(), door.opens().is_some()), 3),
        None => (kit::visitor_mark(), 2 + (scene.tick / 2 % 2) as i32),
    };
    let (vx, vy) = (v.x as i32 - view.x, v.y as i32 - view.y);
    let x = if img.w + 2 > view.w {
        (view.w - img.w) / 2
    } else {
        (vx - img.w / 2).clamp(1, view.w - img.w - 1)
    };
    let above = vy + 2 - 16 - gap - img.h;
    let y = if above >= 1 { above } else { vy + 2 + gap };
    cv.stamp(&img, x, y);
}

#[cfg(test)]
#[path = "../../../tests/world_viz/overworld__visitor_tests.rs"]
mod tests;
